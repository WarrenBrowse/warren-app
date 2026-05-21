#!/usr/bin/env ruby
# frozen_string_literal: true

# C.2.X.B workaround : disable Xcode 15+ explicit-module build mode on
# the project root configurations. The Warren iOS project hits a
# swift-driver race in explicit-module mode where
# `WarrenLogging` cannot resolve the `Logging` swiftmodule emitted by
# the `swift-log` SPM dependency (the module compiles concurrently
# with its consumer). Toggling back to the legacy implicit-module
# build avoids the race at the cost of slightly slower incremental
# builds. To be revisited once the upstream swift-log + Xcode 26.4
# interaction stabilizes.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/disable-explicit-modules.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

changed = []

project.build_configurations.each do |config|
  existing = config.build_settings["SWIFT_ENABLE_EXPLICIT_MODULES"]
  next if existing == "NO"
  config.build_settings["SWIFT_ENABLE_EXPLICIT_MODULES"] = "NO"
  changed << "project / #{config.name}"
end

project.targets.each do |target|
  target.build_configurations.each do |config|
    existing = config.build_settings["SWIFT_ENABLE_EXPLICIT_MODULES"]
    next if existing == "NO"
    config.build_settings["SWIFT_ENABLE_EXPLICIT_MODULES"] = "NO"
    changed << "#{target.name} / #{config.name}"
  end
end

if changed.empty?
  puts "All build configurations already have SWIFT_ENABLE_EXPLICIT_MODULES = NO. No changes."
else
  project.save
  puts "Set SWIFT_ENABLE_EXPLICIT_MODULES = NO on #{changed.size} configurations:"
  changed.first(10).each { |c| puts "  + #{c}" }
  puts "  ..." if changed.size > 10
end
