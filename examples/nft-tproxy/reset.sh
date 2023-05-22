#!/bin/sh
# set -x

RD_FW_MARK="${RD_FW_MARK:=0xfe}"
RD_TABLE="${RD_TABLE:=101}"
RD_DISABLE_IPV6="${RD_DISABLE_IPV6:=0}"

if [ "$(id -u)" != "0" ]; then
   echo "This script must be run as root" 1>&2
   exit 1
fi

nft delete table inet rabbit_digger

ip -4 rule del fwmark $RD_FW_MARK table $RD_TABLE
ip -4 route del local 0/0 dev lo table $RD_TABLE
