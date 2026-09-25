# R17-F15: AGENT.md was not in the markdown set.
CHECK='./scripts/check_doc_identifiers.sh'
EXPECT='PlantedAgentGhost'
plant() {
    append AGENT.md 'The `PlantedAgentGhost` type carries the frame.'
}
