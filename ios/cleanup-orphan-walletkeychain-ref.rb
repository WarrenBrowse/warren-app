#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.3.Z cleanup : after moving WarrenWalletKeychain.swift from
# WarrenVPN/View\ controllers/Wallet/ to Shared/ (git mv), the
# pbxproj still has an orphan PBXFileReference at the old path with
# its associated PBXBuildFile entries. Xcode would fail to compile
# because the old path no longer exists on disk.
#
# This script drops the orphan file_ref + its PBXBuildFile entries.
# The NEW file_ref (in Shared/) is added by add-warren-files-to-target.rb.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/cleanup-orphan-walletkeychain-ref.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

OLD_PATH = "WarrenVPN/View controllers/Wallet/WarrenWalletKeychain.swift"
removed = []

# Find any file_ref pointing at the old path.
project.files.dup.each do |file_ref|
  real = file_ref.real_path.to_s
  next unless real.include?(OLD_PATH)
  # Remove all PBXBuildFile entries that reference this file_ref from
  # every target's source build phase.
  project.targets.each do |target|
    next unless target.respond_to?(:source_build_phase) && target.source_build_phase
    target.source_build_phase.files.dup.each do |build_file|
      next unless build_file.file_ref == file_ref
      target.source_build_phase.files.delete(build_file)
      removed << "build_file: #{target.name} / #{file_ref.display_name}"
    end
  end
  # Drop the file_ref itself.
  file_ref.remove_from_project
  removed << "file_ref: #{real}"
end

if removed.empty?
  puts "No orphan WarrenWalletKeychain refs to clean. Project already clean."
else
  project.save
  puts "Cleaned #{removed.size} orphan entries:"
  removed.each { |e| puts "  - #{e}" }
end
