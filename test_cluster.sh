#!/bin/bash
# Test script for running multiple fastbu instances in cluster mode

# Kill any running instances
pkill -f "target/debug/fastbu"

# Make sure the project is built
echo "Building fastbu..."
cargo build

# Create a cluster config file for each node
cat > node1.toml << EOF
[node]
id = "node1"
host = "127.0.0.1"
port = 7946
api_port = 3031

[cluster]
seeds = []  # First node has no seeds
virtual_nodes = 10
gossip_interval = 1
node_timeout = 10
EOF

cat > node2.toml << EOF
[node]
id = "node2" 
host = "127.0.0.1"
port = 7947  # Different internal port
api_port = 3032

[cluster]
seeds = ["127.0.0.1:7946"]  # Connect to node1
virtual_nodes = 10
gossip_interval = 1
node_timeout = 10
EOF

cat > node3.toml << EOF
[node]
id = "node3"
host = "127.0.0.1"
port = 7948  # Different internal port
api_port = 3033

[cluster]
seeds = ["127.0.0.1:7946"]  # Connect to node1
virtual_nodes = 10
gossip_interval = 1
node_timeout = 10
EOF

# Start the first node (seed node)
echo "Starting seed node (node1) on port 3031 (cluster port 7946)..."
./target/debug/fastbu --cluster --cluster-config node1.toml --host 127.0.0.1 --port 3031 > node1.log 2>&1 &
sleep 2  # Give it time to start

# Start the second node
echo "Starting second node (node2) on port 3032 (cluster port 7947)..."
./target/debug/fastbu --cluster --cluster-config node2.toml --host 127.0.0.1 --port 3032 > node2.log 2>&1 &
sleep 2  # Give it time to start

# Start the third node
echo "Starting third node (node3) on port 3033 (cluster port 7948)..."
./target/debug/fastbu --cluster --cluster-config node3.toml --host 127.0.0.1 --port 3033 > node3.log 2>&1 &
sleep 2  # Give it time to start

echo "Cluster is running with 3 nodes:"
echo "- Node 1: http://127.0.0.1:3031"
echo "- Node 2: http://127.0.0.1:3032"
echo "- Node 3: http://127.0.0.1:3033"

echo ""
echo "You can test with curl commands like:"
echo "  curl -X POST -d 'value=test123' http://127.0.0.1:3031/set/testkey"
echo "  curl http://127.0.0.1:3032/get/testkey"

echo ""
echo "To stop the cluster, run: pkill -f \"target/debug/fastbu\""
