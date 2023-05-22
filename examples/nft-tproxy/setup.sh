#!/bin/sh
# set -x

RD_MARK="${RD_MARK:=0x11}"
RD_PORT="${RD_PORT:=19810}"
RD_PORT6="${RD_PORT6:=19811}"
RD_TABLE="${RD_TABLE:=101}"
RD_FW_MARK="${RD_FW_MARK:=0xfe}"
RD_CT_MARK="${RD_CT_MARK:=0x10}"
RD_INTERNAL_DEV="${RD_INTERNAL_DEV:=br-lan}"
RD_DISABLE_IPV6="${RD_DISABLE_IPV6:=0}"
RD_ENABLE_SELF="${RD_ENABLE_SELF:=0}"
RD_EXCLUDE_IP="$RD_EXCLUDE_IP"
RD_EXCLUDE_MAC="$RD_EXCLUDE_MAC"
RD_HIJACK_DNS="${RD_HIJACK_DNS:=1}"

if [ "$(id -u)" != "0" ]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

# Strategy Route
ip -4 route add local 0/0 dev lo table $RD_TABLE
ip -4 rule add fwmark $RD_FW_MARK table $RD_TABLE

nft "add table inet rabbit_digger"
nft "add chain inet rabbit_digger output { type filter hook output priority raw; policy accept; }"
nft "add chain inet rabbit_digger prerouting { type filter hook prerouting priority mangle; policy accept; }"
nft "add set inet rabbit_digger localnetwork { type ipv4_addr; flags interval; auto-merge; }"
nft "add element inet rabbit_digger localnetwork { 0.0.0.0/8, 127.0.0.0/8, 10.0.0.0/8, 169.254.0.0/16, 192.168.0.0/16, 224.0.0.0/4, 240.0.0.0/4, 172.16.0.0/12}"

# if RD_INTERNAL_DEV is existed
if [ -d /sys/class/net/$RD_INTERNAL_DEV ]; then
   nft "add rule inet rabbit_digger prerouting iifname != $RD_INTERNAL_DEV counter return"
fi
nft "add rule inet rabbit_digger prerouting meta mark $RD_MARK counter return"
nft "add rule inet rabbit_digger prerouting ip daddr @localnetwork return"
nft "add rule inet rabbit_digger prerouting meta l4proto { udp } mark set $RD_FW_MARK tproxy ip to 127.0.0.1:$RD_PORT counter accept"
nft "add rule inet rabbit_digger prerouting meta l4proto { tcp } mark set $RD_FW_MARK tproxy ip to 127.0.0.1:$RD_PORT counter accept"
