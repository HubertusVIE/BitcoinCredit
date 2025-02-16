nssm.exe install "%1" "%CD%\bitcredit.exe" "--http-port %2 --p2p-port 0 --surreal-db-connection rocksdb://data/surreal --nostr-relay wss://bitcr-cloud-run-03-550030097098.europe-west1.run.app --bitcoin-network mainnet"
nssm.exe set "%1" DisplayName "Bitcredit Core"
nssm.exe set "%1" AppStdout "%CD%\logs\stdout.txt"
nssm.exe set "%1" AppStderr "%CD%\logs\stderr.txt"
nssm.exe set "%1" AppRestartDelay 300000
nssm.exe start "%1" "SERVICE_AUTO_START"