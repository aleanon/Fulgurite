# R18-F1, the review's I5a (M42/T42.0): the Everything leg built WITHOUT
# `everything`, so both legs are Boundaries and the run still says "both
# ways". Once fixed, the launcher compares the strategy the PROCESS
# reports with the one the leg declares.
CHECK='./scripts/widget_drive.sh Switch'
EXPECT='expected Everything, got .*Boundaries<256>'
plant() {
    replace scripts/widget_drive.sh 'run_one Everything         agent,everything ' 'run_one Everything         agent '
}
