./nssm.exe install "%2" "%1\bitcredit.exe" "--http-port %3 --p2p-port 0 --surreal-db-connection rocksdb://data/surreal --nostr-relay wss://bitcr-cloud-run-03-550030097098.europe-west1.run.app --bitcoin-network mainnet"
./nssm.exe set "%2" DisplayName "Bitcredit Core"
./nssm.exe set "%2" AppStdout "%1\logs\stdout.txt"
./nssm.exe set "%2" AppStderr "%1\logs\stderr.txt"
./nssm.exe set "%2" AppRestartDelay 300000
./nssm.exe start "%2" "SERVICE_AUTO_START"