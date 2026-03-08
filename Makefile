LOCALDEV_CLUSTER ?= rux
RELEASE_NAME ?= rux
RELEASE_NAMESPACE ?= rux-storage
VERSION ?= $(shell git describe --tags --always --dirty)

KUBECTL ?= kubectl
HELM ?= helm
CARGO ?= cargo
DOCKER_COMPOSE ?= docker compose

SCYLLA_CONTACT_POINTS ?= 127.0.0.1
SCYLLA_LOCAL_DC ?= datacenter1

###
### Build targets
###
.PHONY: build
build:
	$(CARGO) build --release

.PHONY: test
test:
	$(CARGO) test

.PHONY: clippy
clippy:
	$(CARGO) clippy --all-targets --all-features

.PHONY: fmt
fmt:
	$(CARGO) fmt

.PHONY: fmt-check
fmt-check:
	$(CARGO) fmt --check

###
### Local development
###
.PHONY: localdev
localdev: localdev-cluster localdev-cert-manager localdev-scylla-operator localdev-install

.PHONY: localdev-cluster
localdev-cluster:
	@if k3d cluster get $(LOCALDEV_CLUSTER) --no-headers >/dev/null 2>&1; \
		then echo "Cluster '$(LOCALDEV_CLUSTER)' already exists"; \
		else echo "Creating k3d cluster '$(LOCALDEV_CLUSTER)'..." && \
		k3d cluster create --config cluster.yaml --volume $(PWD):/app && \
		for node in $$(docker ps --filter "name=k3d-$(LOCALDEV_CLUSTER)" --format '{{.Names}}'); do \
			docker exec $$node sysctl -w fs.aio-max-nr=1048576 2>/dev/null || true; \
		done; \
	fi

.PHONY: localdev-cert-manager
localdev-cert-manager:
	@echo "Installing cert-manager..."
	@$(HELM) upgrade --install cert-manager oci://quay.io/jetstack/charts/cert-manager \
		--namespace cert-manager --create-namespace \
		--set crds.enabled=true \
		--wait --timeout 120s

.PHONY: localdev-scylla-operator
localdev-scylla-operator:
	@echo "Installing ScyllaDB operator..."
	@$(HELM) repo add scylla-operator https://scylla-operator-charts.storage.googleapis.com/stable 2>/dev/null || true
	@$(HELM) repo update scylla-operator
	@$(HELM) upgrade --install scylla-operator scylla-operator/scylla-operator \
		--namespace scylla-operator --create-namespace \
		--wait --timeout 120s

.PHONY: localdev-install
localdev-install:
	@echo "Installing Rux..."
	@$(HELM) upgrade --install $(RELEASE_NAME) charts/rux \
		--namespace $(RELEASE_NAMESPACE) --create-namespace \
		-f charts/rux/values.localdev.yaml
	@echo "Waiting for Rux gateway to be ready..."
	@$(KUBECTL) wait --for=condition=available --timeout=180s deploy/$(RELEASE_NAME) -n $(RELEASE_NAMESPACE)

.PHONY: localdev-clean
localdev-clean:
	@echo "Deleting k3d cluster '$(LOCALDEV_CLUSTER)'..."
	@k3d cluster delete $(LOCALDEV_CLUSTER)

.PHONY: localdev-restart
localdev-restart: localdev-clean localdev

.PHONY: localdev-uninstall
localdev-uninstall:
	@echo "Uninstalling Rux..."
	@$(HELM) uninstall $(RELEASE_NAME) --namespace $(RELEASE_NAMESPACE)

.PHONY: scylla-up
scylla-up:
	@echo "Starting local ScyllaDB..."
	@$(DOCKER_COMPOSE) up -d scylla
	@echo "Waiting for ScyllaDB healthcheck..."
	@$(DOCKER_COMPOSE) wait scylla >/dev/null || { \
		echo "ScyllaDB did not become healthy in time"; \
		exit 1; \
	}
	@echo "✓ ScyllaDB ready on port 9042"

.PHONY: scylla-down
scylla-down:
	@echo "Stopping local ScyllaDB..."
	@$(DOCKER_COMPOSE) down --remove-orphans
	@echo "✓ ScyllaDB stopped"

.PHONY: scylla-reset
scylla-reset:
	@echo "Stopping local ScyllaDB and removing volumes..."
	@$(DOCKER_COMPOSE) down -v --remove-orphans
	@echo "✓ ScyllaDB data reset"

.PHONY: scylla-logs
scylla-logs:
	@$(DOCKER_COMPOSE) logs -f scylla

.PHONY: scylla-cqlsh-local
scylla-cqlsh-local:
	@docker exec -it rux-scylla cqlsh

.PHONY: run-local
run-local:
	@echo "Running rux against local ScyllaDB..."
	@RUX_SCYLLA_CONTACT_POINTS=$(SCYLLA_CONTACT_POINTS) \
		RUX_SCYLLA_LOCAL_DC=$(SCYLLA_LOCAL_DC) \
		$(CARGO) run

.PHONY: test-integration-local
test-integration-local: scylla-up
	@echo "Running integration tests against local ScyllaDB..."
	@RUX_SCYLLA_CONTACT_POINTS=$(SCYLLA_CONTACT_POINTS) \
		RUX_SCYLLA_LOCAL_DC=$(SCYLLA_LOCAL_DC) \
		$(CARGO) test --tests

.PHONY: dev-local
dev-local: scylla-up
	@echo ""
	@echo "═══════════════════════════════════════════════════"
	@echo "Running rux against local ScyllaDB..."
	@echo "Stop with Ctrl+C to shut down ScyllaDB"
	@echo "═══════════════════════════════════════════════════"
	@echo ""
	@trap '$(MAKE) scylla-down' EXIT; \
	RUX_SCYLLA_CONTACT_POINTS=$(SCYLLA_CONTACT_POINTS) \
	RUX_SCYLLA_LOCAL_DC=$(SCYLLA_LOCAL_DC) \
	$(CARGO) run

.PHONY: test-integration-local-clean
test-integration-local-clean: scylla-up
	@echo "Running integration tests against local ScyllaDB (auto-cleanup)..."
	@trap '$(MAKE) scylla-down' EXIT; \
	RUX_SCYLLA_CONTACT_POINTS=$(SCYLLA_CONTACT_POINTS) \
	RUX_SCYLLA_LOCAL_DC=$(SCYLLA_LOCAL_DC) \
	$(CARGO) test --tests

###
### Dev pod operations
###
.PHONY: run
run: scylla-up
	@echo ""
	@echo "═══════════════════════════════════════════════════"
	@echo "Running rux against local ScyllaDB..."
	@echo "Stop with Ctrl+C to shut down ScyllaDB"
	@echo "═══════════════════════════════════════════════════"
	@echo ""
	@trap '$(MAKE) scylla-down' EXIT; \
	RUX_SCYLLA_CONTACT_POINTS=$(SCYLLA_CONTACT_POINTS) \
	RUX_SCYLLA_LOCAL_DC=$(SCYLLA_LOCAL_DC) \
	$(CARGO) run

.PHONY: run-k8s
run-k8s:
	$(eval POD := $(shell $(KUBECTL) get pods -n $(RELEASE_NAMESPACE) -l app.kubernetes.io/name=rux -o=custom-columns=:metadata.name --no-headers))
	@echo "Running gateway in pod $(POD)..."
	@$(KUBECTL) exec -n $(RELEASE_NAMESPACE) -it pod/$(POD) -- bash -c "cargo run"

.PHONY: exec
exec:
	$(eval POD := $(shell $(KUBECTL) get pods -n $(RELEASE_NAMESPACE) -l app.kubernetes.io/name=rux -o=custom-columns=:metadata.name --no-headers))
	@echo "Connecting to pod $(POD)..."
	@$(KUBECTL) exec -n $(RELEASE_NAMESPACE) -it pod/$(POD) -- bash

.PHONY: logs
logs:
	@$(KUBECTL) logs -n $(RELEASE_NAMESPACE) -l app.kubernetes.io/name=rux -f

.PHONY: scylla-cqlsh
scylla-cqlsh:
	@$(KUBECTL) exec -n scylla -it $(shell $(KUBECTL) get pods -n scylla -l app.kubernetes.io/name=scylla -o=custom-columns=:metadata.name --no-headers | head -1) -- cqlsh

###
### Cluster management
###
.PHONY: cluster-info
cluster-info:
	@k3d cluster list
	@echo ""
	@$(KUBECTL) cluster-info
	@echo ""
	@$(KUBECTL) get nodes

.PHONY: cluster-stop
cluster-stop:
	@k3d cluster stop $(LOCALDEV_CLUSTER)

.PHONY: cluster-start
cluster-start:
	@k3d cluster start $(LOCALDEV_CLUSTER)

###
### Help
###
.PHONY: help
help:
	@echo "Rux Makefile"
	@echo ""
	@echo "Build targets:"
	@echo "  build              - Build release binary"
	@echo "  test               - Run tests"
	@echo "  clippy             - Run clippy linter"
	@echo "  fmt                - Format code"
	@echo "  fmt-check          - Check code formatting"
	@echo ""
	@echo "Local development:"
	@echo "  localdev           - Create k3d cluster + deps + install Rux"
	@echo "  localdev-cluster   - Create k3d cluster only"
	@echo "  localdev-cert-manager - Install cert-manager"
	@echo "  localdev-scylla-operator - Install ScyllaDB operator"
	@echo "  localdev-install   - Install Rux via Helm"
	@echo "  localdev-uninstall - Uninstall Rux"
	@echo "  localdev-clean     - Delete k3d cluster"
	@echo "  localdev-restart   - Delete and recreate cluster"
	@echo "  scylla-up          - Start local ScyllaDB via docker compose"
	@echo "  scylla-down        - Stop local ScyllaDB"
	@echo "  scylla-reset       - Stop local ScyllaDB and delete volumes"
	@echo "  scylla-logs        - Tail local ScyllaDB logs"
	@echo "  scylla-cqlsh-local - Open cqlsh in local ScyllaDB container"
	@echo "  run-local          - Run rux against local ScyllaDB"
	@echo "  dev-local          - Start local ScyllaDB, run rux, auto-stop on exit"
	@echo "  test-integration-local - Run integration tests against local ScyllaDB"
	@echo "  test-integration-local-clean - Run integration tests and auto-stop ScyllaDB"
	@echo "  run                - Run gateway locally with docker compose Scylla (auto-stop on exit)"
	@echo ""
	@echo "Dev pod operations:"
	@echo "  run-k8s            - Run gateway in dev pod"
	@echo "  exec               - Shell into dev pod"
	@echo "  logs               - Tail gateway logs"
	@echo "  scylla-cqlsh       - Open cqlsh to ScyllaDB"
	@echo ""
	@echo "Cluster management:"
	@echo "  cluster-info       - Show cluster information"
	@echo "  cluster-stop       - Stop k3d cluster"
	@echo "  cluster-start      - Start k3d cluster"
	@echo ""
	@echo "Environment variables:"
	@echo "  LOCALDEV_CLUSTER   - Cluster name (default: rux)"
	@echo "  RELEASE_NAME       - Helm release name (default: rux)"
	@echo "  RELEASE_NAMESPACE  - Helm release namespace (default: rux-system)"
	@echo "  SCYLLA_CONTACT_POINTS - Local Scylla contact points for run-local/test (default: 127.0.0.1)"
	@echo "  SCYLLA_LOCAL_DC    - Local Scylla datacenter for run-local/test (default: datacenter1)"
