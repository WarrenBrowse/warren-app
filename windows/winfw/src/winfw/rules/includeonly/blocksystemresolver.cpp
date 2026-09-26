#include "stdafx.h"
#include "blocksystemresolver.h"
#include <winfw/mullvadguids.h>
#include <libwfp/filterbuilder.h>
#include <libwfp/conditionbuilder.h>
#include <libwfp/conditions/comparison.h>
#include <libwfp/conditions/conditioninterface.h>
#include <libwfp/conditions/conditionloopback.h>
#include <libwfp/conditions/conditionport.h>
#include <libwfp/internal/conditionassembler.h>
#include <libcommon/buffer.h>
#include <libcommon/error.h>
#include <sddl.h>
#include <array>
#include <memory>
#include <vector>

using namespace wfp::conditions;

namespace rules::includeonly
{

namespace
{

std::wstring AccountSid(const std::wstring &account)
{
	DWORD sidSize = 0;
	DWORD domainSize = 0;
	SID_NAME_USE use;

	LookupAccountNameW(nullptr, account.c_str(), nullptr, &sidSize, nullptr, &domainSize, &use);

	std::vector<uint8_t> sid(sidSize);
	std::vector<wchar_t> domain(domainSize);

	if (FALSE == LookupAccountNameW(nullptr, account.c_str(), sid.data(), &sidSize, domain.data(), &domainSize, &use))
	{
		THROW_WINDOWS_ERROR(GetLastError(), "Look up the system resolver's account");
	}

	wchar_t *text = nullptr;

	if (FALSE == ConvertSidToStringSidW(sid.data(), &text))
	{
		THROW_WINDOWS_ERROR(GetLastError(), "Format the system resolver's SID");
	}

	std::wstring result(text);
	LocalFree(text);

	return result;
}

//
// Matches the traffic of processes whose token carries the account's SID:
// ALE_USER_ID compares the token against a security descriptor granting the
// match right (FWP_ACTRL_MATCH_FILTER, "CC") to that SID.
//
class ConditionAccount : public IFilterCondition
{
public:

	explicit ConditionAccount(const std::wstring &sid)
		: m_sid(sid)
	{
		const auto sddl = L"O:LSD:(A;;CC;;;" + sid + L")";

		PSECURITY_DESCRIPTOR sd = nullptr;
		ULONG sdSize = 0;

		if (FALSE == ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.c_str(), SDDL_REVISION_1, &sd, &sdSize))
		{
			THROW_WINDOWS_ERROR(GetLastError(), "Build the system resolver's security descriptor");
		}

		FWP_BYTE_BLOB blob;
		blob.size = sdSize;
		blob.data = reinterpret_cast<UINT8 *>(sd);

		m_assembled = wfp::internal::ConditionAssembler::ByteBlob(identifier(), FWP_MATCH_EQUAL, blob);
		LocalFree(sd);

		//
		// A security descriptor is carried as a byte blob with its own type.
		//
		reinterpret_cast<FWPM_FILTER_CONDITION0 *>(m_assembled.data())->conditionValue.type = FWP_SECURITY_DESCRIPTOR_TYPE;
	}

	std::wstring toString() const override
	{
		return L"user = " + m_sid;
	}

	const GUID &identifier() const override
	{
		return FWPM_CONDITION_ALE_USER_ID;
	}

	const FWPM_FILTER_CONDITION0 &condition() const override
	{
		return *reinterpret_cast<const FWPM_FILTER_CONDITION0 *>(m_assembled.data());
	}

private:

	std::wstring m_sid;
	common::Buffer m_assembled;
};

} // anonymous namespace

BlockSystemResolverOffTunnel::BlockSystemResolverOffTunnel(const std::wstring &account, const std::wstring &tunnelInterfaceAlias)
	: m_account(account)
	, m_tunnelInterfaceAlias(tunnelInterfaceAlias)
{
}

//static
const wchar_t *BlockSystemResolverOffTunnel::SystemResolverAccount()
{
	return L"NT SERVICE\\Dnscache";
}

bool BlockSystemResolverOffTunnel::apply(IObjectInstaller &objectInstaller)
{
	const auto sid = AccountSid(m_account);

	const std::array<const GUID *, 2> layers =
	{
		&FWPM_LAYER_ALE_AUTH_CONNECT_V4,
		&FWPM_LAYER_ALE_AUTH_CONNECT_V6,
	};

	for (const auto layer : layers)
	{
		wfp::FilterBuilder filterBuilder(wfp::BuilderValidation::OnlyCritical);

		filterBuilder
			.name(L"Block the system resolver's encrypted DNS outside the tunnel")
			.description(L"This filter is part of a rule that keeps the names looked up for \"VPN only for these apps\" in the tunnel")
			.provider(MullvadGuids::Provider())
			.layer(*layer)
			.sublayer(MullvadGuids::SublayerIncludeOnly())
			.weight(wfp::FilterBuilder::WeightClass::Max)
			.definitive()
			.block();

		wfp::ConditionBuilder conditionBuilder(*layer);

		conditionBuilder.add_condition(std::make_unique<ConditionAccount>(sid));
		conditionBuilder.add_condition(ConditionPort::Remote(443));
		conditionBuilder.add_condition(ConditionPort::Remote(853));
		conditionBuilder.add_condition(std::make_unique<ConditionLoopback>(
			ConditionLoopback::Type::LoopbackTraffic, CompareNeq()));
		conditionBuilder.add_condition(ConditionInterface::Alias(m_tunnelInterfaceAlias, CompareNeq()));

		if (false == objectInstaller.addFilter(filterBuilder, conditionBuilder))
		{
			return false;
		}
	}

	return true;
}

}
