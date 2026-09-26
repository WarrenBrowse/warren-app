# Introduction

`winfw` implements a number of policies for configuring the Windows Filtering Platform (WFP).

# Organization of sublayers

In its initialized state, `winfw` uses a design that involves two different types of sublayers:

- The baseline sublayer
- Other sublayers

When `winfw` is deinitialized, it may create a persistent sublayer to continue applying some policies. Other sublayers and their filters are removed at this time.

## Baseline sublayer

The baseline sublayer is weighted the highest to ensure it sees all traffic first. It contains a large number of permit-filters, with a different subset of them being activated by different policies. The permit-filters are all weighted the same and have the highest possible weight. It doesn't matter which filter sees the traffic first. If traffic is matched by a permit-filter, it's "lifted" out of the sublayer and processing is resumed with the next sublayer.

The baseline sublayer also contains a set of blocking filters that match all traffic. These filters are weighted the lowest within the sublayer. A blocking verdict is final and any traffic matched will be dropped.

The idea is that the primary sublayer (baseline sublayer) shapes the traffic to be more predictable for filters in subsequent sublayers.

## Other sublayers

Beyond the baseline sublayer, there's also the "other" type of sublayer. These sublayers are all weighted the same and slightly lower than the baseline sublayer. These sublayers focus on a specific type of traffic.

Same as the baseline sublayer, these sublayers use a design with highly weighted permit-filters and lower weighted blocking filters.

As an example, we have a sublayer that's dedicated to filtering DNS. Traffic that's not related to DNS will still be sent through it, but all the filters we install must deal only with DNS. This way we can install permit-filters with specific conditions that effectively whitelist the traffic we deem safe. To round it off there's a lower-weighted blocking filter that blocks all DNS.

## Sublayers shared with the split tunnel driver

The split tunnel driver adds its filters to the baseline and DNS sublayers by
keys fixed in its signed binary, and its permits only outweigh the block-all
filters when they sit in the same sublayer. Every other key is salted per
product environment, so that one environment's purge never removes another's
kill switch; these two keys are never salted.

They belong to no provider. An environment creates them when they are missing
and deletes them once no filter uses them, so no environment's teardown fails on
a sublayer another one still has filters in, and none fails to start because
another created them first. Two live policies are never mixed in them: in one
sublayer the permits of either would outweigh the block-all of the other. At
initialization an environment adopts the shared pair only when nothing but its
own filters and the driver's are in it; otherwise it uses private salted keys,
reports it (`WinFw_SplitTunnelSublayersShared`), and the daemon refuses to
engage the split tunnel.

A build that predates the sharing (production's before it) creates its
baseline sublayer at the shared key, owned by its provider, and fails to
start when that key exists: it cannot run next to a sharing build, nor right
after one crashed, until the pair is gone (a reboot removes it, since none of
these objects is persistent).

## Include-only sublayer

"VPN only for these apps" holds the included apps to the tunnel interface and
loopback (`WinFw_SetIncludedApps`) with a hard block in a sublayer of its own,
added to every policy. The driver soft-permits the apps it splits from any
local address but the one physical address it holds, in the blocked states
too; a block in another sublayer outweighs that permit. The same sublayer
carries, in the include-only connected policy, a block of the system
resolver's DNS over HTTPS or TLS off the tunnel (the Dnscache service SID on
ports 443 and 853), which the IPv4 permit outside the tunnel would let out.

## Persistent sublayer

The persistent sublayer is only active when `winfw` is deinitialized and instructed to continue enforcing a blocking policy. It has the highest weight possible and only contains blocking filters that match all traffic. These filters ensure that all traffic will be blocked until `winfw` is reinitialized, including at boot-time before BFE is running. Unlike the other sublayers, this sublayer persists even if BFE is restarted (e.g., by rebooting).

## Advantages of current design

- Predictable sublayer weights.
- Predictable filter weights.
- Short and exact filter condition definitions.
- Removes the need to express logical "and" for same-type conditions, something which is not possible in WFP.
