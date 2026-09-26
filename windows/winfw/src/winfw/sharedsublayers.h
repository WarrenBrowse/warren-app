#pragma once

#include "guidhash.h"
#include "iobjectinstaller.h"
#include <libwfp/filterengine.h>
#include <guiddef.h>
#include <unordered_set>

//
// The baseline and DNS sublayers every product environment shares with the
// split tunnel driver (MullvadGuids::SharedSublayerBaseline, SharedSublayerDns).
//
// They belong to no provider: an environment creates them when they are
// missing and removes them once no filter uses them, so no environment's
// purge ever deletes a sublayer another one still has filters in, and no
// environment fails to start because another one created them first.
//
// Two live policies must still never share them: in one sublayer the
// permits of either would outweigh the block-all of the other. So an
// environment only adopts the shared pair when nothing but its own filters
// and the split tunnel driver's are in it, and uses its private keys
// otherwise (MullvadGuids::UseSharedSublayers).
//
namespace shared_sublayers
{

//
// Whether the shared pair may carry this environment's policy: neither
// sublayer is owned by another provider, and neither holds a filter of any
// provider but `ourProvider` and the split tunnel driver's. Call it after
// our own objects are purged.
//
bool MayAdopt(wfp::FilterEngine &engine, const GUID &ourProvider);

//
// Adds whichever shared sublayer is missing. One that already exists is
// kept as it is.
//
void Install(wfp::FilterEngine &engine);

//
// Adds an inert filter of ours to each shared sublayer. Installed with the
// structural objects, it marks the pair as in use for as long as the context
// lives, policy or not: without it, a sweep (ours or another environment's
// purge) would find the pair unused between policies and delete it, and
// another environment would adopt it.
//
bool Claim(IObjectInstaller &objectInstaller);

//
// Deletes each shared sublayer that no provider owns and no filter uses,
// the filters in `beingRemoved` aside.
//
void RemoveUnused(wfp::FilterEngine &engine, const std::unordered_set<GUID> &beingRemoved);

}
