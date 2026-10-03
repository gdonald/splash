#!/usr/bin/env bash
# Find tests in src/ files that should be moved to tests/ directory.
#
# This script scans Rust source files in src/ and identifies test modules
# (#[cfg(test)] or mod tests) that should be moved to the tests/ directory
# according to the project's testing guidelines.

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."

# Check if src directory exists
SRC_DIR="$PROJECT_ROOT/src"
if [ ! -d "$SRC_DIR" ]; then
  echo "Error: src directory not found"
  exit 1
fi

# Print header
echo "================================================================================"
echo "MISPLACED TESTS REPORT"
echo "================================================================================"
echo ""
echo "According to project guidelines, tests should be in tests/ directory,"
echo "not in implementation files."
echo ""

total_tests=0
files_with_tests=0
declare -a files_info

# Process each Rust file
while IFS= read -r file; do
  if [ ! -f "$file" ]; then
    continue
  fi

  content=$(cat "$file")

  # Check if file has test module
  has_tests=false
  if echo "$content" | grep -qE '#\[cfg\(test\)\]|mod\s+tests\s*\{|mod\s+\w+_tests\s*\{'; then
    has_tests=true
  fi

  if [ "$has_tests" = false ]; then
    continue
  fi

  # Count test functions
  test_count=$(echo "$content" | grep -c '#\[test\]')

  if [ "$test_count" -eq 0 ]; then
    continue
  fi

  # Extract test function names
  test_names=()
  while IFS= read -r line; do
    if [ -n "$line" ]; then
      test_names+=("$line")
    fi
  done < <(echo "$content" | grep -A 1 '#\[test\]' | grep -o 'fn [a-zA-Z_][a-zA-Z0-9_]*' | sed 's/fn //' || true)

  # Store file info
  relative_path="${file#$PROJECT_ROOT/}"
  files_info+=("$relative_path|$test_count|${test_names[*]}")

  total_tests=$((total_tests + test_count))
  files_with_tests=$((files_with_tests + 1))

done < <(find "$SRC_DIR" -type f -name "*.rs" | sort)

# Output results
if [ "$files_with_tests" -gt 0 ]; then
  echo "Found $files_with_tests files with tests ($total_tests total tests):"
  echo ""

  for info in "${files_info[@]}"; do
    IFS='|' read -r file_path test_count test_names_str <<< "$info"

    echo "📄 $file_path"
    echo "   Tests: $test_count"

    if [ -n "$test_names_str" ]; then
      echo "   Functions:"
      read -ra test_names <<< "$test_names_str"
      count=0
      for name in "${test_names[@]}"; do
        if [ "$count" -lt 5 ]; then
          echo "     - $name"
          count=$((count + 1))
        fi
      done

      if [ "${#test_names[@]}" -gt 5 ]; then
        remaining=$((${#test_names[@]} - 5))
        echo "     ... and $remaining more"
      fi
    fi
    echo ""
  done
else
  echo "✅ No tests found in src/ files - all tests are properly placed!"
fi

echo "================================================================================"
echo "Summary: $total_tests tests in $files_with_tests files"
echo "================================================================================"
