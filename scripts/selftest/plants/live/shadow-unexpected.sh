# I-NEW-3, the other direction (M42/T42.0): the Everything leg built WITH
# the oracle. Once fixed, a leg that does not expect it says so.
CHECK='./scripts/agent_drive.sh'
EXPECT='shadow oracle present in a leg that does not expect it'
plant() {
    replace scripts/agent_drive.sh 'run_one Everything agent,everything' 'run_one Everything agent,everything,shadow-oracle'
}
