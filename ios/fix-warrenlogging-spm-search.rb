#!/usr/bin/env ruby
# frozen_string_literal: true

# C.2.X.C : add an explicit `-I` flag to WarrenLogging so its swift
# driver finds the SPM-emitted `Logging.swiftmodule` from swift-log.
# Xcode normally does this automatically via the package_product
# dependency machinery, but a regression introduced by the C.4.4
# WireGuardKit removal (cf. session report) broke the auto-path. This
# script adds a SOURCE_PACKAGES_PATH-anchored `-I` to OTHER_SWIFT_FLAGS.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/fix-warrenlogging-spm-search.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# Path the swift-log SPM package builds its swiftmodule into. Xcode
# expands SOURCE_PACKAGES_PATH per-derived-data ; using the variable
# keeps the project portable.
SPM_BUILD_PATH = "$(SOURCE_PACKAGES_PATH)/checkouts/swift-log/build/$(CONFIGURATION)$(EFFECTIVE_PLATFORM_NAME)"
INCLUDE_FLAG = "-I \"#{SPM_BUILD_PATH}\""

target = project.targets.find { |t| t.name == "WarrenLogging" }
unless target
  warn "WarrenLogging target not found"
  exit 1
end

changed = []
target.build_configurations.each do |config|
  # 1. OTHER_SWIFT_FLAGS : -I for swiftmodule lookup.
  existing = config.build_settings["OTHER_SWIFT_FLAGS"] || "$(inherited)"
  existing = existing.is_a?(Array) ? existing.join(" ") : existing
  unless existing.include?(INCLUDE_FLAG)
    new_value = "#{existing} #{INCLUDE_FLAG}".strip
    config.build_settings["OTHER_SWIFT_FLAGS"] = new_value
    changed << "#{config.name}/OTHER_SWIFT_FLAGS"
  end

  # 2. SWIFT_INCLUDE_PATHS : explicit Swift include path (Xcode
  #    sometimes prefers this over OTHER_SWIFT_FLAGS for module lookup).
  swift_inc = config.build_settings["SWIFT_INCLUDE_PATHS"]
  swift_inc_str = swift_inc.is_a?(Array) ? swift_inc.join(" ") : (swift_inc || "")
  unless swift_inc_str.include?(SPM_BUILD_PATH)
    new_paths = swift_inc_str.empty? ? SPM_BUILD_PATH : "#{swift_inc_str} #{SPM_BUILD_PATH}"
    config.build_settings["SWIFT_INCLUDE_PATHS"] = new_paths
    changed << "#{config.name}/SWIFT_INCLUDE_PATHS"
  end

  # 3. FRAMEWORK_SEARCH_PATHS : Swift modules built by SPM as
  #    `.swiftmodule` may also be packaged as `.framework`, add this
  #    path for completeness.
  fw_paths = config.build_settings["FRAMEWORK_SEARCH_PATHS"]
  fw_paths_str = fw_paths.is_a?(Array) ? fw_paths.join(" ") : (fw_paths || "$(inherited)")
  unless fw_paths_str.include?(SPM_BUILD_PATH)
    new_fw = fw_paths_str.include?("$(inherited)") ? "#{fw_paths_str} #{SPM_BUILD_PATH}" : "$(inherited) #{SPM_BUILD_PATH}"
    config.build_settings["FRAMEWORK_SEARCH_PATHS"] = new_fw.strip
    changed << "#{config.name}/FRAMEWORK_SEARCH_PATHS"
  end
end

if changed.empty?
  puts "WarrenLogging OTHER_SWIFT_FLAGS already includes the SPM include flag. No changes."
else
  project.save
  puts "Added swift-log SPM include flag to WarrenLogging on #{changed.size} configs: #{changed.join(", ")}"
end
