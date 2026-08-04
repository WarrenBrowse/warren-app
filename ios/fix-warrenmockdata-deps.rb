#!/usr/bin/env ruby
# frozen_string_literal: true

# Add explicit target dependencies to WarrenMockData that the Mullvad
# fork relied on via implicit framework links. Xcode 15+ explicit-module
# builds require these to be declared explicitly so Swift module
# resolution finds WarrenREST/Types/Settings/RustRuntime/Logging/Operations
# during WarrenMockData compilation.
#
# Usage: /opt/homebrew/opt/ruby/bin/ruby ios/fix-warrenmockdata-deps.rb
# Idempotent : checks existing dependencies before adding.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

mockdata = project.targets.find { |t| t.name == "WarrenMockData" }
unless mockdata
  warn "WarrenMockData target not found"
  exit 1
end

# Modules imported by WarrenMockData/**/*.swift (cf. `import ... ; @testable
# import ... ;` directives). Each must be an explicit target dependency
# so the explicit-module build sees the .swiftmodule + -Swift.h before
# compilation of WarrenMockData starts.
REQUIRED_DEPS = %w[
  WarrenREST
  WarrenTypes
  WarrenSettings
  WarrenRustRuntime
  WarrenLogging
  Operations
].freeze

added = []
skipped = []

REQUIRED_DEPS.each do |dep_name|
  dep_target = project.targets.find { |t| t.name == dep_name }
  unless dep_target
    warn "Target #{dep_name.inspect} not found, skipping"
    next
  end
  already_depends = mockdata.dependencies.any? do |dep|
    dep.target == dep_target ||
      (dep.target_proxy && dep.target_proxy.remote_global_id_string == dep_target.uuid)
  end
  if already_depends
    skipped << dep_name
    next
  end
  mockdata.add_dependency(dep_target)
  added << dep_name
end

if added.empty?
  puts "All WarrenMockData dependencies already declared. No changes."
else
  project.save
  puts "Added #{added.size} dependencies to WarrenMockData:"
  added.each { |d| puts "  + #{d}" }
  puts "Skipped (already present): #{skipped.size}"
  skipped.each { |d| puts "  = #{d}" }
end
