#!/bin/bash
set -e
# Modify from: https://github.com/shadowsocks/shadowsocks-rust/blob/0b1630d1c6abcec3861b1eec39b266e1dad206e5/configs/iptables_tproxy.sh
# RD_MARK, RD_FW_MARK and RD_PORT are changed to avoid conflict with origin iptables rules.

RD_MARK=0xfe
RD_PORT=19810
RD_FW_MARK=0x1919
RD_TABLE=100

if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

# Strategy Route
ip -4 route add local 0/0 dev lo table $RD_TABLE
ip -4 rule add fwmark $RD_FW_MARK table $RD_TABLE

iptables -t mangle -N RD
# Reserved addresses
iptables -t mangle -A RD -d 0/8 -j RETURN
iptables -t mangle -A RD -d 127/8 -j RETURN
iptables -t mangle -A RD -d 10/8 -j RETURN
iptables -t mangle -A RD -d 169.254/16 -j RETURN
iptables -t mangle -A RD -d 172.16/12 -j RETURN
iptables -t mangle -A RD -d 192.168/16 -j RETURN
iptables -t mangle -A RD -d 224/4 -j RETURN
iptables -t mangle -A RD -d 240/4 -j RETURN

# TPROXY TCP/UDP mark RD_FW_MARK to port RD_PORT
iptables -t mangle -A RD -p udp -j TPROXY --on-port $RD_PORT --tproxy-mark $RD_FW_MARK
iptables -t mangle -A RD -p tcp -j TPROXY --on-port $RD_PORT --tproxy-mark $RD_FW_MARK

# Apply
iptables -t mangle -A PREROUTING -j RD

# OUTPUT rules
iptables -t mangle -N RD-MARK
# Reserved addresses
iptables -t mangle -A RD-MARK -d 0/8 -j RETURN
iptables -t mangle -A RD-MARK -d 127/8 -j RETURN
iptables -t mangle -A RD-MARK -d 10/8 -j RETURN
iptables -t mangle -A RD-MARK -d 169.254/16 -j RETURN
iptables -t mangle -A RD-MARK -d 172.16/12 -j RETURN
iptables -t mangle -A RD-MARK -d 192.168/16 -j RETURN
iptables -t mangle -A RD-MARK -d 224/4 -j RETURN
iptables -t mangle -A RD-MARK -d 240/4 -j RETURN

# Bypass out-going with mask RD_MARK
iptables -t mangle -A RD-MARK -j RETURN -m mark --mark $RD_MARK

# Reroute
iptables -t mangle -A RD-MARK -p udp -j MARK --set-mark $RD_FW_MARK
iptables -t mangle -A RD-MARK -p tcp -j MARK --set-mark $RD_FW_MARK

# Apply
iptables -t mangle -A OUTPUT -j RD-MARK
