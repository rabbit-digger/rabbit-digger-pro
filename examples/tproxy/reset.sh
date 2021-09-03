#!/bin/sh
# set -x

RD_FW_MARK="${RD_FW_MARK:=0xfe}"
RD_TABLE="${RD_TABLE:=101}"
RD_DISABLE_IPV6="${RD_DISABLE_IPV6:=0}"

if [ "$(id -u)" != "0" ]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

if [ "$RD_DISABLE_IPV6" != "1" ]; then
   ip6tables -t mangle -D OUTPUT -j RD_OUTPUT
   ip6tables -t mangle -D PREROUTING -j RD_PREROUTING

   ip6tables -t mangle -F RD_PREROUTING
   ip6tables -t mangle -X RD_PREROUTING

   ip6tables -t mangle -F RD_OUTPUT
   ip6tables -t mangle -X RD_OUTPUT

   ip -6 rule del fwmark $RD_FW_MARK table $RD_TABLE
   ip -6 route del local ::/0 dev lo table $RD_TABLE
fi

iptables -t mangle -D OUTPUT -j RD_OUTPUT
iptables -t mangle -D PREROUTING -j RD_PREROUTING

iptables -t mangle -F RD_PREROUTING
iptables -t mangle -X RD_PREROUTING

iptables -t mangle -F RD_OUTPUT
iptables -t mangle -X RD_OUTPUT

ip -4 rule del fwmark $RD_FW_MARK table $RD_TABLE
ip -4 route del local 0/0 dev lo table $RD_TABLE
