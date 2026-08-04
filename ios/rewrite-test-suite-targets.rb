#!/usr/bin/env ruby
# frozen_string_literal: true

# Test-suite realignment for the wallet + payment model (task #7).
#
# 1. Removes the genuinely-dead Mullvad account-number UI tests from BOTH
#    the Xcode project AND disk:
#      - WarrenVPNUITests/AccountTests.swift           (login(accountNumber:), getAccountNumber, device mgmt)
#      - WarrenVPNUITests/Payment/PaymentTests.swift   (IAP add-time via the Mullvad account flow)
#    These exercise the removed account-number identity and cannot be
#    faithfully ported (the wallet model has no account number, no partner
#    API temporary accounts, no device management). The remaining
#    account-coupled UITests (ConnectivityTests/SettingsMigrationTests/
#    RelayTests) are NOT touched: they are backend-dependent and mostly
#    salvageable, so they are left for a sim+backend rewrite.
#
# 2. Registers the new test files into their correct targets:
#      - Payment UNIT tests -> WarrenVPNTests (runs in CI)
#      - Wallet-flow UI tests + page objects -> WarrenVPNUITests
#
# Idempotent: re-running once files are added / removed is a no-op.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")

# Dead account-number UI tests: removed from project + disk.
DEAD_FILES = [
  "WarrenVPNUITests/AccountTests.swift",
  "WarrenVPNUITests/Payment/PaymentTests.swift",
].freeze

# New files -> target. Paths are relative to `ios/`.
FILES_TO_ADD = {
  # Payment unit tests (WarrenVPNTests is a HOSTED target compiling app
  # sources via @testable import WarrenVPN, so these reach the
  # StorePaymentManager types directly).
  "WarrenVPNTests/MullvadVPN/StorePaymentManager/StoreSubscriptionTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/StorePaymentManager/StorePaymentOutcomeTests.swift" => "WarrenVPNTests",
  "WarrenVPNTests/MullvadVPN/StorePaymentManager/StorePaymentManagerInteractorTests.swift" => "WarrenVPNTests",

  # Wallet-flow UI tests + page objects.
  "WarrenVPNUITests/WalletFlowTests.swift" => "WarrenVPNUITests",
  "WarrenVPNUITests/Pages/WalletLoginPage.swift" => "WarrenVPNUITests",
  "WarrenVPNUITests/Pages/WalletBackupPage.swift" => "WarrenVPNUITests",
}.freeze

# Find or create a PBXGroup at the relative path. Walks the group tree
# under the project main group and creates missing intermediate groups.
def find_or_create_group(project, relative_path)
  parts = relative_path.split(File::SEPARATOR)
  group = project.main_group
  parts.each do |segment|
    next if segment.empty?
    child = group.children.find { |c| c.is_a?(Xcodeproj::Project::Object::PBXGroup) && c.display_name == segment }
    child = group.new_group(segment, segment) if child.nil?
    group = child
  end
  group
end

project = Xcodeproj::Project.open(PROJECT_PATH)
project_root = File.expand_path(SCRIPT_DIR)

removed_build_files = []
removed_file_refs = []
deleted_from_disk = []

DEAD_FILES.each do |rel_path|
  abs_path = File.join(project_root, rel_path)
  refs = project.files.select { |f| f.real_path.to_s == abs_path }

  refs.each do |file_ref|
    project.targets.each do |target|
      next unless target.respond_to?(:source_build_phase)
      phases = [target.source_build_phase]
      phases << target.resources_build_phase if target.respond_to?(:resources_build_phase)
      phases.compact.each do |phase|
        phase.files.select { |bf| bf.file_ref == file_ref }.each do |bf|
          phase.remove_build_file(bf)
          removed_build_files << "#{rel_path} from #{target.name}"
        end
      end
    end

    file_ref.remove_from_project
    removed_file_refs << rel_path
  end

  if File.exist?(abs_path)
    File.delete(abs_path)
    deleted_from_disk << rel_path
  end
end

added = []
skipped = []

FILES_TO_ADD.each do |rel_path, target_name|
  target = project.targets.find { |t| t.name == target_name }
  if target.nil?
    warn "Target #{target_name.inspect} not found, skipping #{rel_path}"
    next
  end

  abs_path = File.join(project_root, rel_path)
  existing_ref = project.files.find { |f| f.real_path.to_s == abs_path }
  if existing_ref
    skipped << rel_path
    unless target.source_build_phase.files_references.include?(existing_ref)
      target.add_file_references([existing_ref])
    end
    next
  end

  parent_group = find_or_create_group(project, File.dirname(rel_path))
  file_ref = parent_group.new_reference(File.basename(rel_path))
  target.add_file_references([file_ref])
  added << "#{rel_path} -> #{target_name}"
end

if removed_build_files.empty? && removed_file_refs.empty? && deleted_from_disk.empty? &&
   added.empty? && skipped.size == FILES_TO_ADD.size
  puts "Nothing to do: dead files already gone and new files already present."
else
  project.save

  unless removed_build_files.empty?
    puts "Removed #{removed_build_files.size} build-file entry/entries:"
    removed_build_files.each { |d| puts "  - #{d}" }
  end
  unless removed_file_refs.empty?
    puts "Removed #{removed_file_refs.size} file reference(s):"
    removed_file_refs.each { |d| puts "  - #{d}" }
  end
  unless deleted_from_disk.empty?
    puts "Deleted #{deleted_from_disk.size} file(s) from disk:"
    deleted_from_disk.each { |d| puts "  - #{d}" }
  end
  unless added.empty?
    puts "Added #{added.size} file(s):"
    added.each { |p| puts "  + #{p}" }
  end
  unless skipped.empty?
    puts "Skipped (already present): #{skipped.size}"
    skipped.each { |p| puts "  = #{p}" }
  end
end
