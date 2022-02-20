iperf3 -s &
cargo run --release -- -c ./benchmark.yaml &


echo "Waiting for iperf3 to start"
while ! nc -z 127.0.0.1 5201; do   
  sleep 1
done
echo "iperf3 is started"

echo "Waiting for RDP to start"

while ! nc -z 127.0.0.1 20000; do   
  sleep 1
done

echo "Start benchmark"

mkdir -p ./results

TCP_FLAGS="-t 10"
UDP_FLAGS="-u -b 1G -l 1000"

iperf3 -c 127.0.0.1 $TCP_FLAGS > ./results/iperf3.txt
iperf3 -c 127.0.0.1 $UDP_FLAGS >> ./results/iperf3.txt

iperf3 -c 127.0.0.1 -p 20000 $TCP_FLAGS >  ./results/iperf3-forward.txt
iperf3 -c 127.0.0.1 -p 20000 $UDP_FLAGS >> ./results/iperf3-forward.txt

iperf3 -c 127.0.0.1 -p 20001 $TCP_FLAGS >  ./results/iperf3-aes-128-gcm.txt
iperf3 -c 127.0.0.1 -p 20001 $UDP_FLAGS >> ./results/iperf3-aes-128-gcm.txt

iperf3 -c 127.0.0.1 -p 20002 $TCP_FLAGS >  ./results/iperf3-aes-256-gcm.txt
iperf3 -c 127.0.0.1 -p 20002 $UDP_FLAGS >> ./results/iperf3-aes-256-gcm.txt

iperf3 -c 127.0.0.1 -p 20003 $TCP_FLAGS >  ./results/iperf3-chacha20-ietf-poly1305.txt
iperf3 -c 127.0.0.1 -p 20003 $UDP_FLAGS >> ./results/iperf3-chacha20-ietf-poly1305.txt


kill -INT %1
kill -9 %2

echo "benchmark finished"

jobs
