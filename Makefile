.PHONY: all build test clean format lint check coverage run docker-build docker-run

# Default target
all: check

# Build the project
build:
	cargo build --verbose

# Build the project in release mode
build-release:
	cargo build --release --verbose

# Run tests
test:
	cargo test --verbose

# Clean build artifacts
clean:
	cargo clean
	rm -rf cache_storage

# Format code
format:
	cargo fmt --all

# Check formatting
format-check:
	cargo fmt --all -- --check

# Run clippy linter
lint:
	cargo clippy -- -D warnings

# Run all checks (format and lint)
check: format-check lint

# Generate code coverage
coverage:
	cargo install cargo-llvm-cov
	cargo llvm-cov --html

# Run the application
run:
	cargo run

# Run the application with specific port
run-port:
	cargo run -- -p $(PORT)

# Build Docker image
docker-build:
	docker build -t fastbu .

# Run Docker container
docker-run:
	docker run -p 3031:3031 fastbu

# Install development dependencies
install-dev-deps:
	rustup component add rustfmt clippy llvm-tools-preview

# Help target
help:
	@echo "Available targets:"
	@echo "  all           - Run all checks (default)"
	@echo "  build         - Build the project"
	@echo "  build-release - Build the project in release mode"
	@echo "  test          - Run tests"
	@echo "  clean         - Clean build artifacts"
	@echo "  format        - Format code"
	@echo "  format-check  - Check formatting"
	@echo "  lint          - Run clippy linter"
	@echo "  check         - Run all checks (format and lint)"
	@echo "  coverage      - Generate code coverage"
	@echo "  run           - Run the application"
	@echo "  run-port      - Run the application with specific port (e.g., make run-port PORT=8080)"
	@echo "  docker-build  - Build Docker image"
	@echo "  docker-run    - Run Docker container"
	@echo "  install-dev-deps - Install development dependencies"
	@echo "  help          - Show this help message" 