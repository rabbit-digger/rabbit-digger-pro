#!/bin/bash
set -e

RD_FW_MARK=0x1919
RD_TABLE=100

if [[ $EUID -ne 0 ]]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

iptables -t mangle -D OUTPUT -j RD-MARK

iptables -t mangle -F RD-MARK
iptables -t mangle -X RD-MARK

iptables -t mangle -D PREROUTING -j RD

iptables -t mangle -F RD
iptables -t mangle -X RD

ip -4 rule del fwmark $RD_FW_MARK table $RD_TABLE
ip -4 route del local 0/0 dev lo table $RD_TABLE
