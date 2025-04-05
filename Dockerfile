# Use the official Rust image as the base image
FROM rust:1.81 as builder

# Set the working directory inside the container
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Pre-build dependencies
RUN cargo build --release && rm -rf src

# Copy the source code into the container
COPY . .

# Build the application
RUN cargo build --release

# Use a minimal base image for the final container
FROM rust:1.81

# Set the working directory inside the container
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/fastbu .

# Expose the port the server will run on
EXPOSE 3030

# Run the application
CMD ["./fastbu"]