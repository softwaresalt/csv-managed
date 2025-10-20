.\target\release\csv-managed.exe probe -i .\tests\data\big_5_players_stats_2023_2024.csv -m .\tests\data\big_5_players_stats.schema --mapping --replace
.\target\release\csv-managed.exe verify -m .\tests\data\big_5_players_stats.schema -i .\tests\data\big_5_players_stats_2023_2024.csv

