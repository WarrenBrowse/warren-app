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
	static RemovalFunctor GetRemoveAllGenerationsFunctor(const std::vector<uint32_t> &salts);

	static bool Execute(RemovalFunctor f);
};
