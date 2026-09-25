# R17-F1: a file its cfg compiles out lists zero tests and nothing noticed.
# With the recipe line that runs `golden` alone removed, the zero-test check
# must name it (T43.9).
CHECK='./scripts/check_test_counts.sh'
EXPECT='golden'
plant() {
    drop_lines justfile '--test golden'
}
