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
  "WarrenVPN/View controllers/Wallet/WarrenWalletKeychain.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenMnemonicInputView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenMnemonicDisplayView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletInteractor.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletGenerateViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletImportViewController.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Wallet/WarrenWalletBackupViewController.swift" => "WarrenVPN",
  "WarrenVPN/Coordinators/WarrenWalletCoordinator.swift" => "WarrenVPN",

  # Settings + Tunnel Warren-specific views.
  "WarrenVPN/View controllers/Settings/WarrenMultiHopSettingsView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenObfuscationIndicatorView.swift" => "WarrenVPN",

  # WarrenRustRuntime FFI wrappers.
  "WarrenRustRuntime/WarrenWallet.swift" => "WarrenRustRuntime",
  "WarrenRustRuntime/WarrenQuinnAdapter.swift" => "WarrenRustRuntime",

  # C.6 remainder: DAITA / NAT-PMP settings + failover banner +
  # App Group event observer wired into TunnelViewController.
  "WarrenVPN/View controllers/Settings/WarrenDaitaSettingsView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Settings/WarrenNatPmpSettingsView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenFailoverBannerView.swift" => "WarrenVPN",
  "WarrenVPN/View controllers/Tunnel/WarrenAppGroupEvents.swift" => "WarrenVPN",

  # Onboarding wizard 5-step (SwiftUI) + coordinator.
  "WarrenVPN/View controllers/Onboarding/OnboardingWizardView.swift" => "WarrenVPN",
  "WarrenVPN/Coordinators/OnboardingWizardCoordinator.swift" => "WarrenVPN",

  # Unit tests for the wallet flow + App Group events observer (run on
  # iOS Simulator).
  "WarrenVPNTests/MullvadVPN/Wallet/WarrenWalletKeychainTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/Wallet/WarrenWalletTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/View controllers/Tunnel/WarrenAppGroupEventsTests.swift" =>
    "WarrenVPNTests",

  # i18n resources: Wallet + Settings + Onboarding tables for FR + EN.
  "Assets/Wallet.xcstrings" => "WarrenVPN",
  "Assets/Settings.xcstrings" => "WarrenVPN",
  "Assets/Onboarding.xcstrings" => "WarrenVPN",
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

  is_resource = rel_path.end_with?(".xcstrings", ".plist", ".png", ".pdf")

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

if added.empty? && skipped.size == FILES_TO_ADD.size
  puts "All files already present in the project. No changes."
else
  project.save
  puts "Added #{added.size} files:"
  added.each { |p| puts "  + #{p}" }
  puts "Skipped (already present): #{skipped.size}"
  skipped.each { |p| puts "  = #{p}" }
end
