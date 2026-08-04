#!/usr/bin/env ruby
# frozen_string_literal: true

# Fixes stale PBXGroup paths under the WarrenVPNTests group. The top-level
# test groups were renamed Mullvad*->Warren* in the project but the on-disk
# directories stayed Mullvad*, so the whole WarrenVPNTests target failed to
# build ("Build input file cannot be found: .../WarrenVPNTests/WarrenVPN/...").
# This only rewrites a group's `path` from Warren* to Mullvad* when the
# current path does not resolve on disk AND the Mullvad-swapped directory
# does, so app/framework groups (which resolve correctly) are never touched.
#
# Usage: ruby ios/fix-warrenvpntests-group-paths.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

tests_group = project.groups.find { |g| g.path == "WarrenVPNTests" }
unless tests_group
  warn "WarrenVPNTests group not found"
  exit 1
end

fixed = []

tests_group.children.each do |child|
  next unless child.is_a?(Xcodeproj::Project::Object::PBXGroup)
  next unless child.path&.start_with?("Warren")
  next if File.directory?(child.real_path)

  candidate = child.path.sub(/\AWarren/, "Mullvad")
  candidate_path = File.join(tests_group.real_path.to_s, candidate)
  next unless File.directory?(candidate_path)

  child.path = candidate
  fixed << "#{child.display_name}: -> path=#{candidate}"
end

if fixed.empty?
  puts "No stale WarrenVPNTests group paths. No changes."
else
  project.save
  puts "Fixed #{fixed.size} group paths:"
  fixed.each { |f| puts "  #{f}" }
end
