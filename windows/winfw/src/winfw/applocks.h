#pragma once

#include "objectpurger.h"
#include <optional>
#include <string>
#include <vector>

namespace applocks
{

//
// Replaces the app locks with `apps`, in one transaction of its own: the
// locks belong to no policy and need no initialized context. Zero apps (or
// none that resolves) remove every lock object, provider included.
//
bool Apply(
	const std::vector<std::wstring> &apps,
	const std::optional<std::wstring> &tunnelInterfaceAlias,
	bool permitLan
);

//
// Removes every app lock object of this environment.
//
ObjectPurger::RemovalFunctor GetRemoveFunctor();

}
