# I-NEW-3 (M42/T42.0): agent_drive.sh's Boundaries leg built WITHOUT the
# shadow oracle. `if sh.get("supported"):` had no else, so the leg passed
# having compared nothing. Once fixed, a leg that expects the oracle
# refuses a build without it ("cannot say" is never "no").
CHECK='./scripts/agent_drive.sh'
EXPECT='expected the shadow oracle in this build'
plant() {
    replace scripts/agent_drive.sh 'run_one Boundaries agent,shadow-oracle' 'run_one Boundaries agent'
}
