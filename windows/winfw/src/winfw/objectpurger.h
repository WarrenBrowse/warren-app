#pragma once

#include "winfw.h"
#include "libwfp/filterengine.h"
#include <cstdint>
#include <functional>
#include <vector>

class ObjectPurger
{
public:

	ObjectPurger() = delete;

	using RemovalFunctor = std::function<void(wfp::FilterEngine &engine)>;

	static RemovalFunctor GetRemoveAllFunctor();
	static RemovalFunctor GetRemoveNonPersistentFunctor();

	//
	// Recovery sweep: removes our objects under EVERY listed environment salt,
	// not just the one this build was compiled for. Blocking objects keyed for
	// an environment the machine no longer runs are invisible to the normal
	// purge, so without this they can never be removed by the product.
	//
	// `removedObjects`, when non-null, receives the number of filters and
	// sublayers that matched and were removed. The caller uses it to say out
	// loud that the machine WAS carrying orphaned firewall state: a non-zero
	// sweep at startup is the trace of a host that was (or was about to be)
	// blocked by objects nothing else could remove.
	//
	static RemovalFunctor GetRemoveAllGenerationsFunctor(
		const std::vector<uint32_t> &salts,
		uint32_t *removedObjects = nullptr);

	static bool Execute(RemovalFunctor f);
};
