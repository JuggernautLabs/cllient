#!/bin/bash
# Script to run v1-v2 equivalence tests
#
# Prerequisites:
# - pkg-config must be installed
# - OpenSSL development libraries must be installed
#   Ubuntu/Debian: apt-get install pkg-config libssl-dev
#   Fedora/RHEL: yum install pkg-config openssl-devel
#   macOS: brew install pkg-config openssl
#
# Usage:
#   chmod +x run_v1_v2_equivalence_tests.sh
#   ./run_v1_v2_equivalence_tests.sh

set -e

echo "Running v1-v2 equivalence tests..."
echo "=================================="
echo ""

# Run the equivalence tests
cargo test --test v1_v2_equivalence_test -- --nocapture

echo ""
echo "=================================="
echo "All equivalence tests passed!"
