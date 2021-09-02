#!/bin/sh
# set -x

RD_MARK="${RD_MARK:=0x11}"
RD_PORT="${RD_PORT:=19810}"
RD_TABLE="${RD_TABLE:=101}"
RD_FW_MARK="${RD_FW_MARK:=0xfe}"
RD_CT_MARK="${RD_CT_MARK:=0x10}"
RD_INTERNAL_DEV="${RD_INTERNAL_DEV:=br-lan}"

if [ "$(id -u)" != "0" ]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

# Strategy Route
ip -4 route add local 0/0 dev lo table $RD_TABLE
ip -4 rule add fwmark $RD_FW_MARK table $RD_TABLE

iptables -t mangle -N RD_OUTPUT
# if RD_INTERNAL_DEV is existed
if [ -n /sys/class/net/$RD_INTERNAL_DEV ]; then
   iptables -t mangle -A RD_OUTPUT ! -i $RD_INTERNAL_DEV -j RETURN
else
   echo "Warning: Internet interface `$RD_INTERNAL_DEV` is not found, the traffic may mess up."
fi
iptables -t mangle -A RD_OUTPUT -s 0/8 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 127/8 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 10/8 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 169.254/16 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 172.16/12 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 192.168/16 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 224/4 -j RETURN
iptables -t mangle -A RD_OUTPUT -s 240/4 -j RETURN
iptables -t mangle -A RD_OUTPUT -j RETURN -m mark --mark $RD_MARK
iptables -t mangle -A RD_OUTPUT -p udp -j MARK --set-mark $RD_FW_MARK
iptables -t mangle -A RD_OUTPUT -p tcp -j MARK --set-mark $RD_FW_MARK

iptables -t mangle -N RD_PREROUTING
# if RD_INTERNAL_DEV is existed
if [ -e /sys/class/net/$RD_INTERNAL_DEV ]; then
   iptables -t mangle -A RD_PREROUTING ! -i $RD_INTERNAL_DEV -j RETURN
fi
iptables -t mangle -A RD_PREROUTING -d 0/8 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 127/8 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 10/8 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 169.254/16 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 172.16/12 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 192.168/16 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 224/4 -j RETURN
iptables -t mangle -A RD_PREROUTING -d 240/4 -j RETURN
iptables -t mangle -A RD_PREROUTING -m mark --mark $RD_MARK -j RETURN
iptables -t mangle -A RD_PREROUTING -j TPROXY -p udp --on-port $RD_PORT --tproxy-mark $RD_FW_MARK
iptables -t mangle -A RD_PREROUTING -j TPROXY -p tcp --on-port $RD_PORT --tproxy-mark $RD_FW_MARK

iptables -t mangle -A OUTPUT -j RD_OUTPUT
iptables -t mangle -A PREROUTING -j RD_PREROUTING
