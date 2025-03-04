RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --enable-private-ip \
    --sdn-port 10001 \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 1 \
    --seeds-from-url "http://localhost:8080/api/cluster/seeds?zone_id=0&node_type=Gateway" \
    --workers 2 \
    gateway \
        --lat 10 \
        --lon 20 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
