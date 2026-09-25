# R17-F15: the allowlist was subtracted and never itself checked -- six of
# its entries were live code tokens at 1717808, `PanicReport` among them,
# whose own entry said it "comes off when it exists".
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='PanicReport'
plant() {
    append scripts/doc-identifiers.allow 'PanicReport'
}
