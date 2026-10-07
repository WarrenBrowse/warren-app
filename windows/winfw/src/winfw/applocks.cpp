#include "stdafx.h"
#include "applocks.h"
#include "iobjectinstaller.h"
#include "mullvadguids.h"
#include "mullvadobjects.h"
#include "rules/applocks/locktotunnel.h"
#include "rules/includeonly/blockoutsidetunnel.h"
#include <libwfp/filterengine.h>
#include <libwfp/objectdeleter.h>
#include <libwfp/objectenumerator.h>
#include <libwfp/objectinstaller.h>
#include <libwfp/transaction.h>
#include <unordered_set>

namespace applocks
{

namespace
{

class EngineInstaller : public IObjectInstaller
{
public:

	explicit EngineInstaller(wfp::FilterEngine &engine)
		: m_engine(engine)
	{
	}

	bool addProvider(wfp::ProviderBuilder &providerBuilder) override
	{
		return wfp::ObjectInstaller::AddProvider(m_engine, providerBuilder);
	}

	bool addSublayer(wfp::SublayerBuilder &sublayerBuilder) override
	{
		return wfp::ObjectInstaller::AddSublayer(m_engine, sublayerBuilder);
	}

	bool addFilter(wfp::FilterBuilder &filterBuilder, const wfp::IConditionBuilder &conditionBuilder) override
	{
		return wfp::ObjectInstaller::AddFilter(m_engine, filterBuilder, conditionBuilder);
	}

private:

	wfp::FilterEngine &m_engine;
};

void RemoveAll(wfp::FilterEngine &engine)
{
	std::unordered_set<GUID> filtersToRemove;

	wfp::ObjectEnumerator::Filters(engine, [&](const auto &filter) -> bool
	{
		if (nullptr != filter.providerKey && *filter.providerKey == MullvadGuids::ProviderAppLocks())
		{
			filtersToRemove.insert(filter.filterKey);
		}
		return true;
	});

	for (const auto &filter : filtersToRemove)
	{
		wfp::ObjectDeleter::DeleteFilter(engine, filter);
	}

	wfp::ObjectDeleter::DeleteSublayer(engine, MullvadGuids::SublayerAppLocks());
	wfp::ObjectDeleter::DeleteProvider(engine, MullvadGuids::ProviderAppLocks());
}

} // anonymous namespace

bool Apply(
	const std::vector<std::wstring> &apps,
	const std::optional<std::wstring> &tunnelInterfaceAlias,
	bool permitLan
)
{
	auto engine = wfp::FilterEngine::StandardSession();

	return wfp::Transaction::Execute(*engine, [&]()
	{
		RemoveAll(*engine);

		if (rules::includeonly::ResolvableApps(apps).empty())
		{
			return true;
		}

		EngineInstaller installer(*engine);

		return installer.addProvider(*MullvadObjects::ProviderAppLocks())
			&& installer.addSublayer(*MullvadObjects::SublayerAppLocks())
			&& rules::applocks::LockToTunnel(apps, tunnelInterfaceAlias, permitLan).apply(installer);
	});
}

ObjectPurger::RemovalFunctor GetRemoveFunctor()
{
	return [](wfp::FilterEngine &engine)
	{
		RemoveAll(engine);
	};
}

}
