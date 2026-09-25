# R17-F11: ci.yml's comments stated counts nothing checked ("six crates"
# listing seven names).
CHECK='./scripts/check_ci_claims.sh'
EXPECT='\.github/workflows/ci\.yml:[0-9]+'
plant() {
    append .github/workflows/ci.yml '# the dependency tree is six crates'
}
