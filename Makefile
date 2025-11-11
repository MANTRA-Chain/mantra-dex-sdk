# Makefile for Mantra SDK Development and Deployment

.PHONY: help build test clean docker-build docker-up docker-down k8s-deploy k8s-delete lint format check

# Default target
help: ## Show this help message
	@echo "Mantra SDK - Available Commands:"
	@echo
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# Build targets
build: ## Build the project with all features
	cargo build --features mcp,performance,security,resilience

build-release: ## Build release version
	cargo build --release --features mcp,performance,security,resilience

build-tui: ## Build TUI variant
	cargo build --bin mantra-dex-tui --features tui-dex

build-all: ## Build all variants
	@echo "Building MCP server..."
	cargo build --release --bin mcp-server --features mcp,performance,security,resilience
	@echo "Building TUI..."
	cargo build --release --bin mantra-dex-tui --features tui-dex

# Test targets
test: ## Run all tests
	cargo test --features mcp,performance,security,resilience

test-unit: ## Run unit tests only
	cargo test --lib --features mcp,performance,security,resilience

test-integration: ## Run integration tests only
	cargo test --test integration_test --features mcp,performance,security,resilience

test-tui: ## Run TUI tests
	cargo test --features tui-dex

test-coverage: ## Generate test coverage report
	cargo tarpaulin --verbose --all-features --workspace --timeout 120 \
		--exclude-files src/bin/* \
		--out html --output-dir ./coverage

# Code quality targets
lint: ## Run clippy linter
	cargo clippy --all-targets --all-features -- -D warnings

format: ## Format code
	cargo fmt --all

format-check: ## Check code formatting
	cargo fmt --all -- --check

check: ## Run cargo check
	cargo check --all-targets --all-features

audit: ## Security audit
	cargo audit

outdated: ## Check for outdated dependencies
	cargo outdated

# AST-grep semantic analysis
ast-scan: ## Run all AST-grep semantic analysis rules
	@command -v ast-grep >/dev/null 2>&1 || { echo "ast-grep not installed. Run: cargo install ast-grep"; exit 1; }
	ast-grep scan

ast-security: ## Check security patterns with AST-grep
	@command -v ast-grep >/dev/null 2>&1 || { echo "ast-grep not installed. Run: cargo install ast-grep"; exit 1; }
	ast-grep scan -c rules/security.yml

ast-errors: ## Check error handling patterns
	@command -v ast-grep >/dev/null 2>&1 || { echo "ast-grep not installed. Run: cargo install ast-grep"; exit 1; }
	ast-grep scan -c rules/error-handling.yml

ast-protocols: ## Check protocol implementation patterns
	@command -v ast-grep >/dev/null 2>&1 || { echo "ast-grep not installed. Run: cargo install ast-grep"; exit 1; }
	ast-grep scan -c rules/protocol-patterns.yml

ast-quality: ## Check code quality patterns
	@command -v ast-grep >/dev/null 2>&1 || { echo "ast-grep not installed. Run: cargo install ast-grep"; exit 1; }
	ast-grep scan -c rules/code-quality.yml

ast-todos: ## Find all TODO/FIXME comments
	@echo "=== TODO Comments ==="
	@grep -rn "TODO:" src/ tests/ 2>/dev/null || echo "No TODO comments found"
	@echo ""
	@echo "=== FIXME Comments ==="
	@grep -rn "FIXME:" src/ tests/ 2>/dev/null || echo "No FIXME comments found"

# Clean targets
clean: ## Clean build artifacts
	cargo clean
	docker system prune -f

clean-all: ## Clean everything including Docker volumes
	cargo clean
	docker-compose down -v
	docker system prune -a -f --volumes

# Docker targets
docker-build: ## Build Docker images
	docker build -t mantra-sdk:mcp-server --target mcp-server .
	docker build -t mantra-sdk:tui --target tui .
	docker build -t mantra-sdk:development --target development .

docker-build-dev: ## Build development Docker image
	docker build -t mantra-sdk:dev --target development .

docker-up: ## Start Docker Compose services
	docker-compose up -d

docker-down: ## Stop Docker Compose services
	docker-compose down

docker-logs: ## Show Docker Compose logs
	docker-compose logs -f

docker-restart: ## Restart Docker Compose services
	docker-compose restart

docker-rebuild: ## Rebuild and restart services
	docker-compose down
	docker-compose build --no-cache
	docker-compose up -d

# Development targets
dev: ## Start development environment
	docker-compose -f docker-compose.yml up -d redis prometheus grafana
	cargo run --bin mcp-server --features mcp,performance,security,resilience

dev-tui: ## Start TUI in development mode
	cargo run --bin mantra-dex-tui --features tui-dex

dev-watch: ## Start with auto-reload
	cargo watch -x "run --bin mcp-server --features mcp,performance,security,resilience"

dev-full: ## Start full development stack
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# Kubernetes targets
k8s-deploy: ## Deploy to Kubernetes
	kubectl apply -f k8s/deployment.yaml
	kubectl apply -f k8s/monitoring.yaml

k8s-delete: ## Delete Kubernetes deployment
	kubectl delete -f k8s/deployment.yaml
	kubectl delete -f k8s/monitoring.yaml

k8s-status: ## Check Kubernetes deployment status
	kubectl get pods -n mantra-sdk
	kubectl get services -n mantra-sdk

k8s-logs: ## Show Kubernetes logs
	kubectl logs -f deployment/mcp-server -n mantra-sdk

k8s-restart: ## Restart Kubernetes deployment
	kubectl rollout restart deployment/mcp-server -n mantra-sdk

k8s-scale: ## Scale deployment (usage: make k8s-scale REPLICAS=5)
	kubectl scale deployment/mcp-server --replicas=$(REPLICAS) -n mantra-sdk

# Production deployment targets
deploy-staging: ## Deploy to staging environment
	@echo "Deploying to staging..."
	cd k8s/environments/staging && kustomize build . | kubectl apply -f -
	kubectl rollout status deployment/mcp-server -n mantra-sdk-staging --timeout=300s

deploy-production: ## Deploy to production environment
	@echo "Deploying to production..."
	@read -p "Are you sure you want to deploy to production? [y/N] " confirm; \
	if [ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ]; then \
		cd k8s/environments/production && kustomize build . | kubectl apply -f -; \
		kubectl rollout status deployment/mcp-server -n mantra-sdk --timeout=600s; \
	else \
		echo "Deployment cancelled."; \
	fi

# Monitoring targets
monitoring-up: ## Start monitoring stack
	docker-compose up -d prometheus grafana jaeger

monitoring-down: ## Stop monitoring stack
	docker-compose stop prometheus grafana jaeger

# Security targets
security-scan: ## Run security scan on Docker image
	docker run --rm -v /var/run/docker.sock:/var/run/docker.sock \
		-v $(PWD):/tmp aquasec/trivy image mantra-sdk:mcp-server

security-check: ## Run security checks
	cargo audit
	cargo deny check

# Benchmarking targets
bench: ## Run benchmarks
	cargo bench --features mcp,performance,security,resilience

bench-report: ## Generate benchmark report
	cargo bench --features mcp,performance,security,resilience -- --output-format html

# Release targets
release-patch: ## Create patch release
	cargo release patch --execute

release-minor: ## Create minor release
	cargo release minor --execute

release-major: ## Create major release
	cargo release major --execute

# Documentation targets
docs: ## Generate documentation
	cargo doc --all-features --no-deps --document-private-items

docs-open: ## Generate and open documentation
	cargo doc --all-features --no-deps --document-private-items --open

# Performance testing
perf-test: ## Run performance tests
	@echo "Starting performance test environment..."
	docker-compose up -d mcp-server redis
	@echo "Waiting for services to be ready..."
	sleep 30
	@echo "Running performance tests..."
	k6 run tests/performance/load-test.js
	docker-compose down

# Environment setup
setup: ## Setup development environment
	@echo "Setting up development environment..."
	rustup update
	rustup component add rustfmt clippy
	cargo install cargo-watch cargo-audit cargo-outdated cargo-tarpaulin cargo-release
	@echo "Creating necessary directories..."
	mkdir -p logs data coverage
	@echo "Setup complete!"

# Quick commands
quick-test: format-check lint test ## Run quick quality checks

full-check: quick-test test-coverage audit ast-scan-ci ## Run comprehensive checks

ast-scan-ci: ## Run AST-grep if available (CI-friendly)
	@if command -v ast-grep >/dev/null 2>&1; then \
		echo "Running AST-grep semantic analysis..."; \
		ast-grep scan; \
	else \
		echo "⚠️  AST-grep not installed, skipping semantic analysis"; \
		echo "Install with: cargo install ast-grep"; \
	fi

ci-build: clean build-all test-integration ## Simulate CI build

# Variables
REPLICAS ?= 3
IMAGE_TAG ?= latest
NAMESPACE ?= mantra-sdk

# Include local overrides if they exist
-include Makefile.local