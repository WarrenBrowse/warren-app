#include "stdafx.h"
#include "sharedsublayers.h"
#include "mullvadguids.h"
#include <libwfp/objectdeleter.h>
#include <libwfp/objectenumerator.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/conditions/conditionport.h>
#include <libwfp/conditions/conditionprotocol.h>
#include <libcommon/error.h>
#include <fwpmu.h>
#include <array>
#include <optional>

namespace shared_sublayers
{

namespace
{

constexpr GUID NoProvider = { 0 };

struct SharedSublayer
{
	const GUID &key;
	const wchar_t *name;
	const wchar_t *description;
	UINT16 weight;
};

std::array<SharedSublayer, 2> Sublayers()
{
	//
	// Same names and weights as the private sublayers they stand in for.
	//
	return { {
		{ MullvadGuids::SharedSublayerBaseline(), L"Mullvad VPN baseline", L"Filters that enforce a good baseline", MAXUINT16 },
		{ MullvadGuids::SharedSublayerDns(), L"Mullvad VPN DNS", L"Filters that restrict DNS traffic", MAXUINT16 - 1 },
	} };
}

bool IsShared(const GUID &sublayerKey)
{
	return sublayerKey == MullvadGuids::SharedSublayerBaseline()
		|| sublayerKey == MullvadGuids::SharedSublayerDns();
}

//
// The owner of the sublayer at `key`: nullopt when it does not exist, a null
// GUID when it exists and belongs to no provider.
//
std::optional<GUID> Owner(wfp::FilterEngine &engine, const GUID &key)
{
	FWPM_SUBLAYER0 *sublayer = nullptr;

	const auto status = FwpmSubLayerGetByKey0(engine.session(), &key, &sublayer);

	if (FWP_E_SUBLAYER_NOT_FOUND == status)
	{
		return std::nullopt;
	}

	if (ERROR_SUCCESS != status)
	{
		THROW_WINDOWS_ERROR(status, "Read shared sublayer");
	}

	const GUID owner = (nullptr == sublayer->providerKey) ? NoProvider : *sublayer->providerKey;

	FwpmFreeMemory0(reinterpret_cast<void **>(&sublayer));

	return owner;
}

} // anonymous namespace

bool MayAdopt(wfp::FilterEngine &engine, const GUID &ourProvider)
{
	for (const auto &sublayer : Sublayers())
	{
		const auto owner = Owner(engine, sublayer.key);

		if (owner.has_value() && NoProvider != *owner && ourProvider != *owner)
		{
			return false;
		}
	}

	bool foreignFilter = false;

	wfp::ObjectEnumerator::Filters(engine, [&](const FWPM_FILTER0 &filter) -> bool
	{
		if (false == IsShared(filter.subLayerKey))
		{
			return true;
		}

		const bool ours = nullptr != filter.providerKey
			&& (ourProvider == *filter.providerKey
				|| MullvadGuids::SplitTunnelDriverProvider() == *filter.providerKey);

		foreignFilter = !ours;

		return !foreignFilter;
	});

	return !foreignFilter;
}

void Install(wfp::FilterEngine &engine)
{
	for (const auto &shared : Sublayers())
	{
		FWPM_SUBLAYER0 sublayer = { 0 };

		sublayer.subLayerKey = shared.key;
		sublayer.displayData.name = const_cast<wchar_t *>(shared.name);
		sublayer.displayData.description = const_cast<wchar_t *>(shared.description);
		sublayer.weight = shared.weight;

		const auto status = FwpmSubLayerAdd0(engine.session(), &sublayer, nullptr);

		if (ERROR_SUCCESS != status && FWP_E_ALREADY_EXISTS != status)
		{
			THROW_WINDOWS_ERROR(status, "Register shared sublayer with BFE");
		}
	}
}

bool Claim(IObjectInstaller &objectInstaller)
{
	const std::array<std::pair<const GUID *, const GUID *>, 2> claims =
	{ {
		{ &MullvadGuids::Filter_SharedSublayer_Claim_Baseline(), &MullvadGuids::SharedSublayerBaseline() },
		{ &MullvadGuids::Filter_SharedSublayer_Claim_Dns(), &MullvadGuids::SharedSublayerDns() },
	} };

	for (const auto &[filterKey, sublayerKey] : claims)
	{
		wfp::FilterBuilder filterBuilder;

		//
		// Blocks only TCP to port 0, which no connection uses, so it can match
		// nothing that matters whatever other filters share the sublayer.
		//
		filterBuilder
			.key(*filterKey)
			.name(L"Claim a sublayer shared with the split tunnel driver")
			.description(L"This filter marks the sublayer as in use by this firewall; it matches no real traffic")
			.provider(MullvadGuids::Provider())
			.layer(FWPM_LAYER_ALE_AUTH_CONNECT_V4)
			.sublayer(*sublayerKey)
			.weight(wfp::FilterBuilder::WeightClass::Min)
			.block();

		wfp::ConditionBuilder conditionBuilder(FWPM_LAYER_ALE_AUTH_CONNECT_V4);

		conditionBuilder.add_condition(wfp::conditions::ConditionProtocol::Tcp());
		conditionBuilder.add_condition(wfp::conditions::ConditionPort::Remote(0));

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

void RemoveUnused(wfp::FilterEngine &engine, const std::unordered_set<GUID> &beingRemoved)
{
	std::unordered_set<GUID> inUse;

	wfp::ObjectEnumerator::Filters(engine, [&](const FWPM_FILTER0 &filter) -> bool
	{
		if (IsShared(filter.subLayerKey) && beingRemoved.end() == beingRemoved.find(filter.filterKey))
		{
			inUse.insert(filter.subLayerKey);
		}

		return true;
	});

	for (const auto &sublayer : Sublayers())
	{
		if (inUse.end() != inUse.find(sublayer.key))
		{
			continue;
		}

		//
		// A shared key owned by a provider was made by a product that does not
		// share it (a build predating this, or a foreign one): only its owner
		// may delete it.
		//
		const auto owner = Owner(engine, sublayer.key);

		if (owner.has_value() && NoProvider == *owner)
		{
			wfp::ObjectDeleter::DeleteSublayer(engine, sublayer.key);
		}
	}
}

}
