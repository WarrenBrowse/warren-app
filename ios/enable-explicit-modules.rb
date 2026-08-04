#!/usr/bin/env ruby
# frozen_string_literal: true

# Inverse of `disable-explicit-modules.rb`. Removes the
# SWIFT_ENABLE_EXPLICIT_MODULES override (lets Xcode use its default,
# which is YES in Xcode 15+). Provided as a §0.0-INVIOLABLE-safe
# alternative to `git checkout` for reverting the disable script's
# effect when testing build behavior.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/enable-explicit-modules.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

changed = []

project.build_configurations.each do |config|
  next unless config.build_settings.key?("SWIFT_ENABLE_EXPLICIT_MODULES")
  config.build_settings.delete("SWIFT_ENABLE_EXPLICIT_MODULES")
  changed << "project / #{config.name}"
end

project.targets.each do |target|
  next unless target.respond_to?(:build_configurations)
  target.build_configurations.each do |config|
    next unless config.build_settings.key?("SWIFT_ENABLE_EXPLICIT_MODULES")
    config.build_settings.delete("SWIFT_ENABLE_EXPLICIT_MODULES")
    changed << "#{target.name} / #{config.name}"
  end
end

if changed.empty?
  puts "No SWIFT_ENABLE_EXPLICIT_MODULES overrides to remove. Project already clean."
else
  project.save
  puts "Removed SWIFT_ENABLE_EXPLICIT_MODULES override on #{changed.size} configurations."
end
