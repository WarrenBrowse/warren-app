#include "stdafx.h"
#include "blockoutsidetunnel.h"
#include <winfw/mullvadguids.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/conditions/comparison.h>
#include <libwfp/conditions/conditionapplication.h>
#include <libwfp/conditions/conditioninterface.h>
#include <libwfp/conditions/conditionloopback.h>
#include <libwfp/internal/conditionassembler.h>
#include <libcommon/buffer.h>
#include <libcommon/error.h>
#include <array>
#include <cwctype>
#include <memory>

using namespace wfp::conditions;

namespace rules::includeonly
{

namespace
{

bool IsDevicePath(const std::wstring &path)
{
	static const std::wstring prefix = L"\\device\\";

	if (path.size() < prefix.size())
	{
		return false;
	}

	for (size_t i = 0; i < prefix.size(); ++i)
	{
		if (std::towlower(path[i]) != prefix[i])
		{
			return false;
		}
	}

	return true;
}

//
// The app id of an executable the split tunnel driver reports by its NT
// device path. An app id is that path lowercased, null included, which
// FwpmGetAppIdFromFileName0 derives from a DOS path; it cannot take the
// device path itself.
//
class ConditionDeviceApplication : public IFilterCondition
{
public:

	explicit ConditionDeviceApplication(const std::wstring &devicePath)
		: m_devicePath(devicePath)
	{
		//
		// Lowercased by Unicode rules, as the app ids the filtering engine
		// derives: a path outside ASCII would never match otherwise.
		//
		std::wstring appId(devicePath);

		if (0 == LCMapStringEx(LOCALE_NAME_INVARIANT, LCMAP_LOWERCASE, devicePath.c_str(),
			static_cast<int>(devicePath.size()), appId.data(), static_cast<int>(appId.size()),
			nullptr, nullptr, 0))
		{
			THROW_WINDOWS_ERROR(GetLastError(), "Lowercase an app's device path");
		}

		FWP_BYTE_BLOB blob;
		blob.size = static_cast<UINT32>((appId.size() + 1) * sizeof(wchar_t));
		blob.data = reinterpret_cast<UINT8 *>(const_cast<wchar_t *>(appId.c_str()));

		m_assembled = wfp::internal::ConditionAssembler::ByteBlob(identifier(), FWP_MATCH_EQUAL, blob);
	}

	std::wstring toString() const override
	{
		return L"application = " + m_devicePath;
	}

	const GUID &identifier() const override
	{
		return FWPM_CONDITION_ALE_APP_ID;
	}

	const FWPM_FILTER_CONDITION0 &condition() const override
	{
		return *reinterpret_cast<const FWPM_FILTER_CONDITION0 *>(m_assembled.data());
	}

private:

	std::wstring m_devicePath;
	common::Buffer m_assembled;
};

std::unique_ptr<IFilterCondition> AppCondition(const std::wstring &app)
{
	if (IsDevicePath(app))
	{
		return std::make_unique<ConditionDeviceApplication>(app);
	}

	return std::make_unique<ConditionApplication>(app);
}

std::vector<std::wstring> ResolvableApps(const std::vector<std::wstring> &apps)
{
	std::vector<std::wstring> resolvable;

	for (const auto &app : apps)
	{
		try
		{
			AppCondition(app);
			resolvable.push_back(app);
		}
		catch (...)
		{
		}
	}

	return resolvable;
}

} // anonymous namespace

BlockOutsideTunnel::BlockOutsideTunnel(
	const std::vector<std::wstring> &apps,
	const std::optional<std::wstring> &tunnelInterfaceAlias
)
	: m_apps(apps)
	, m_tunnelInterfaceAlias(tunnelInterfaceAlias)
{
}

bool BlockOutsideTunnel::apply(IObjectInstaller &objectInstaller)
{
	const auto apps = ResolvableApps(m_apps);

	if (apps.empty())
	{
		return true;
	}

	const std::array<const GUID *, 4> layers =
	{
		&FWPM_LAYER_ALE_AUTH_CONNECT_V4,
		&FWPM_LAYER_ALE_AUTH_CONNECT_V6,
		&FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4,
		&FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V6,
	};

	for (const auto layer : layers)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		filterBuilder
			.name(L"Block the apps of \"VPN only for these apps\" outside the tunnel")
			.description(L"This filter is part of a rule that holds the included apps to the tunnel and loopback")
			.provider(MullvadGuids::Provider())
			.layer(*layer)
			.sublayer(MullvadGuids::SublayerIncludeOnly())
			.weight(wfp::FilterBuilder::WeightClass::Max)
			.definitive()
			.block();

		wfp::ConditionBuilder conditionBuilder(*layer);

		for (const auto &app : apps)
		{
			conditionBuilder.add_condition(AppCondition(app));
		}

		conditionBuilder.add_condition(std::make_unique<ConditionLoopback>(
			ConditionLoopback::Type::LoopbackTraffic, CompareNeq()));

		if (m_tunnelInterfaceAlias.has_value())
		{
			conditionBuilder.add_condition(ConditionInterface::Alias(*m_tunnelInterfaceAlias, CompareNeq()));
		}

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

}
