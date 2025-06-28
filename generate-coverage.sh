#!/bin/bash
echo "Generating Rust test coverage..."
cargo llvm-cov --lcov --output-path lcov.info
cargo llvm-cov --html
echo "Coverage report generated!"
echo "LCOV file: lcov.info"
echo "HTML report: target/llvm-cov/html/index.html"
echo ""
echo "In VS Code:"
echo "1. Open Command Palette (Ctrl+Shift+P)"
echo "2. Run 'Coverage Gutters: Display Coverage'"
echo "3. Coverage will be shown as colored bars in the editor gutters"
