#!/usr/bin/env ruby
# frozen_string_literal: true

# Adds Warren-side Swift files to the WarrenVPN.xcodeproj WarrenVPN
# and WarrenRustRuntime targets. Idempotent: skips files already
# present in the project.
#
# Usage: ruby ios/add-warren-files-to-target.rb
#
# Run from the repo root or from `ios/`. Locates the .xcodeproj based
# on the script directory.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# Map of (file path relative to `ios/`) -> target name.
FILES_TO_ADD = {
  # UI appearance / brand tokens (consumed by both UIKit + SwiftUI).
  "WarrenVPN/UI appearance/UIColor+Warren.swift" => "WarrenVPN",

  # Wallet flow (UIKit shells + SwiftUI hosted views + Interactor + Keychain).
  # WarrenWalletKeychain moved to Shared/ (C.4.3.Z) so PacketTunnel
  # extension can read the wallet seed for actor.bindWalletSigningSeed.
  # Kept the WarrenVPN entry plus added PacketTunnelCore + PacketTunnel
  # via SHARED_MULTI_TARGET below.
  "Shared/WarrenWalletKeychain.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenMnemonicInputView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenMnemonicDisplayView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletInteractor.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletGenerateViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletImportViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletBackupViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletEraseViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletIdentityView.swift" => "WarrenVPN",
  "WarrenVPN/Coordinators/WarrenWalletCoordinator.swift" => "WarrenVPN",

  # P3.1 wallet-as-login: Create/Restore login coordinator + logout-wipe helper.
  "WarrenVPN/Coordinators/WarrenWalletLoginCoordinator.swift" => "WarrenVPN",
  "WarrenVPN/Classes/WarrenWalletLogout.swift" => "WarrenVPN",

  # Settings + Tunnel Warren-specific views.
  "WarrenVPN/View controllers/Tunnel/WarrenObfuscationIndicatorView.swift" => "WarrenVPN",

  # C.4.3 Warren Quinn tunnel implementation + actor scaffold inside
  # `PacketTunnelCore` (slots next to GotaTunTunnelImplementation +
  # GotaTunActor, conforms `TunnelImplementation` + `PacketTunnelActorProtocol`).
  "PacketTunnelCore/Actor/WarrenQuinnActor.swift" => "PacketTunnelCore",
  "PacketTunnelCore/Actor/WarrenQuinnTunnelImplementation.swift" => "PacketTunnelCore",

  # C.4.3.X follow-up : shared App Group key constants consumed by both
  # the PacketTunnel extension (producer) and the main app
  # (consumer). Wired into WarrenVPN target alongside the existing
  # Shared/* files (ApplicationConfiguration etc.).
  "Shared/WarrenAppGroupKey.swift" => "WarrenVPN",

  # WarrenRustRuntime FFI wrappers.
  "WarrenRustRuntime/WarrenWallet.swift" => "WarrenRustRuntime",
  "WarrenRustRuntime/WarrenAccountClient.swift" => "WarrenRustRuntime",
  "WarrenRustRuntime/WarrenQuinnAdapter.swift" => "WarrenRustRuntime",

  # C.4.5 partial : no-op TunnelObfuscation stub. Provides the
  # namespace + protocol + enum surface that ProtocolObfuscator
  # consumes, without the Rust FFI dep (Warren's HTTP/3 mimicry is
  # baked into the warren-tunnel Quinn layer, not a local proxy).
  "WarrenRustRuntime/TunnelObfuscationTypes.swift" => "WarrenRustRuntime",

  # NAT-PMP settings + failover banner + App Group event observer
  # wired into TunnelViewController. (The old WarrenDaita/WarrenMultiHop
  # settings scaffolds were deleted from disk; do not re-add refs to
  # missing files or the build fails with "Build input files cannot
  # be found".)
  "WarrenVPN/View controllers/Settings/WarrenNatPmpSettingsView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenFailoverBannerView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenAppGroupEvents.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenTunnelStatisticsView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Settings/WarrenDiagnosticInfoView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Settings/WarrenAboutView.swift" => "WarrenVPN",

  # Onboarding wizard 5-step (SwiftUI) + coordinator.
  "WarrenVPN/View controllers/Onboarding/OnboardingWizardView.swift" => "WarrenVPN",
  "WarrenVPN/Coordinators/OnboardingWizardCoordinator.swift" => "WarrenVPN",

  # Unit tests for the wallet flow + App Group events observer (run on
  # iOS Simulator).
  "WarrenVPNTests/MullvadVPN/Wallet/WarrenWalletKeychainTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Wallet/WarrenWalletTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Wallet/WarrenWalletInteractorTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/View controllers/Tunnel/WarrenAppGroupEventsTests.swift" =>
    "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Shared/WarrenAppGroupKeyTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/View controllers/Tunnel/WarrenTunnelStatisticsViewTests.swift" =>
    "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/View controllers/Settings/WarrenDiagnosticInfoViewTests.swift" =>
    "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/View controllers/Settings/WarrenAboutViewTests.swift" =>
    "WarrenVPNTests",
  "PacketTunnelCoreTests/WarrenQuinnActorTests.swift" => "PacketTunnelCoreTests",

  # P3.1 wallet-as-login unit tests: DeviceState synthesizer + route
  # decision. WarrenWalletRoutingTests references `nextWarrenRoutes`/
  # `AppRoute`; it compiles now that WarrenVPNTests is a HOSTED target
  # (TEST_HOST=WarrenVPN.app) and reaches app internals via
  # `@testable import WarrenVPN`.
  "WarrenVPNTests/MullvadVPN/WarrenWallet/WarrenWalletDeviceStateTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/WarrenWallet/WarrenWalletRoutingTests.swift" => "WarrenVPNTests",

  # i18n resources: Wallet + Settings + Onboarding tables for FR + EN.
  "Assets/Wallet.xcstrings" => "WarrenVPN",
  "Assets/Settings.xcstrings" => "WarrenVPN",
  "Assets/Onboarding.xcstrings" => "WarrenVPN",

  # Forced-update gate (signed ios.json manifest check): Rust verifier
  # facade, gate service, blocking screen, unit tests + signed fixture.
  "WarrenRustRuntime/WarrenVersionCheck.swift" => "WarrenRustRuntime",
  "WarrenVPN/Classes/WarrenAppVersionGate.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/BlockedUpdate/WarrenBlockedUpdateView.swift" => "WarrenVPN",
  "WarrenVPNTests/MullvadVPN/WarrenVersionGate/WarrenAppVersionGateTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/WarrenVersionGate/ios-manifest.json" => "WarrenVPNTests",

  # Community-forum sign-in (doc 55): the product table read from Rust, the
  # forum identity store, the login flow, the sign-in code screen, the
  # account row, and their tests. The fixture loader is shared by both test
  # bundles (SHARED_MULTI_TARGET below).
  "WarrenRustRuntime/WarrenProductAnchors.swift" => "WarrenRustRuntime",
  "WarrenRustRuntime/WarrenForumIdentityStore.swift" => "WarrenRustRuntime",
  "WarrenVPN/Classes/WarrenForumLogin.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Settings/WarrenForumSignInCodeView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Account/ForumHandleRow.swift" => "WarrenVPN",
  "WarrenVPNTests/Fixtures/ClientRulesFixtures.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Forum/WarrenForumLinkTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Forum/WarrenForumLoginCopyTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Forum/WarrenForumIdentityStoreTests.swift" => "WarrenVPNTests",
  "WarrenRustRuntimeTests/WarrenProductAnchorsTests.swift" => "WarrenRustRuntimeTests",

  # The non-prod markers: the name iOS gives the VPN configuration in
  # Settings, General, VPN and Device Management, and the header chip.
  "WarrenVPNTests/MullvadVPN/TunnelManager/TunnelConfigurationTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Classes/HeaderBarViewTests.swift" => "WarrenVPNTests",
}.freeze

# Files that need to live in MULTIPLE targets (mirrors the Mullvad
# `Shared/` pattern : each target compiles its own copy via PBXBuildFile
# entries pointing at the same file_ref). Hash key is path ; value is
# the array of *additional* target names beyond what FILES_TO_ADD
# declares.
SHARED_MULTI_TARGET = {
  "Shared/WarrenAppGroupKey.swift" => %w[PacketTunnelCore PacketTunnel],
  "Shared/WarrenWalletKeychain.swift" => %w[PacketTunnelCore PacketTunnel],
  "WarrenVPNTests/Fixtures/ClientRulesFixtures.swift" => %w[WarrenRustRuntimeTests],
}.freeze

# Find or create a PBXGroup at the relative path. Walks the group tree
# under the project main group and creates missing intermediate groups.
def find_or_create_group(project, relative_path)
  parts = relative_path.split(File::SEPARATOR)
  group = project.main_group
  parts.each do |segment|
    next if segment.empty?
    child = group.children.find { |c| c.is_a?(Xcodeproj::Project::Object::PBXGroup) && c.display_name == segment }
    if child.nil?
      child = group.new_group(segment, segment)
    end
    group = child
  end
  group
end

added = []
skipped = []

FILES_TO_ADD.each do |rel_path, target_name|
  target = project.targets.find { |t| t.name == target_name }
  if target.nil?
    warn "Target #{target_name.inspect} not found, skipping #{rel_path}"
    next
  end

  is_resource = rel_path.end_with?(".xcstrings", ".plist", ".png", ".pdf", ".json")

  # Check if a file ref with this path already exists anywhere.
  existing_ref = project.files.find { |f| f.real_path.to_s == File.join(SCRIPT_DIR, rel_path) }
  if existing_ref
    skipped << rel_path
    if is_resource
      unless target.resources_build_phase.files_references.include?(existing_ref)
        target.resources_build_phase.add_file_reference(existing_ref)
      end
    else
      unless target.source_build_phase.files_references.include?(existing_ref)
        target.add_file_references([existing_ref])
      end
    end
    next
  end

  # Determine the parent group from the path (everything except the filename).
  parent_dir = File.dirname(rel_path)
  parent_group = find_or_create_group(project, parent_dir)

  # Add file reference under the parent group.
  file_ref = parent_group.new_reference(File.basename(rel_path))

  if is_resource
    target.resources_build_phase.add_file_reference(file_ref)
  else
    target.add_file_references([file_ref])
  end

  added << rel_path
end

# Add SHARED_MULTI_TARGET extra wires after primary pass so file_refs
# always exist (so we hit the `existing_ref` branch above and just
# attach to the secondary target's source build phase).
SHARED_MULTI_TARGET.each do |rel_path, extra_targets|
  existing_ref = project.files.find { |f| f.real_path.to_s == File.join(SCRIPT_DIR, rel_path) }
  unless existing_ref
    warn "SHARED_MULTI_TARGET: file_ref for #{rel_path} not yet present, skipping"
    next
  end
  extra_targets.each do |target_name|
    target = project.targets.find { |t| t.name == target_name }
    if target.nil?
      warn "SHARED_MULTI_TARGET: target #{target_name} not found, skipping"
      next
    end
    next if target.source_build_phase.files_references.include?(existing_ref)
    target.add_file_references([existing_ref])
    added << "#{rel_path} -> #{target_name}"
  end
end

if added.empty? && skipped.size == FILES_TO_ADD.size
  puts "All files already present in the project. No changes."
else
  project.save
  puts "Added #{added.size} files:"
  added.each { |p| puts "  + #{p}" }
  puts "Skipped (already present): #{skipped.size}"
  skipped.each { |p| puts "  = #{p}" }
end
