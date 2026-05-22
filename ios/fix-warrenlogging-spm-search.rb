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
INCLUDE_FLAG = "-I \"$(SOURCE_PACKAGES_PATH)/checkouts/swift-log/build/$(CONFIGURATION)$(EFFECTIVE_PLATFORM_NAME)\""

target = project.targets.find { |t| t.name == "WarrenLogging" }
unless target
  warn "WarrenLogging target not found"
  exit 1
end

changed = []
target.build_configurations.each do |config|
  existing = config.build_settings["OTHER_SWIFT_FLAGS"] || "$(inherited)"
  existing = existing.is_a?(Array) ? existing.join(" ") : existing
  next if existing.include?(INCLUDE_FLAG)
  new_value = "#{existing} #{INCLUDE_FLAG}".strip
  config.build_settings["OTHER_SWIFT_FLAGS"] = new_value
  changed << config.name
end

if changed.empty?
  puts "WarrenLogging OTHER_SWIFT_FLAGS already includes the SPM include flag. No changes."
else
  project.save
  puts "Added swift-log SPM include flag to WarrenLogging on #{changed.size} configs: #{changed.join(", ")}"
end
