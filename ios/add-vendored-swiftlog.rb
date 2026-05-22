#!/usr/bin/env ruby
# frozen_string_literal: true

# C.2.X.C : add the 4 vendored swift-log source files (copied to
# WarrenLogging/Vendor/SwiftLog/) to the WarrenLogging target's
# source build phase. Once added, the `Logger` / `LogHandler` /
# `Metadata` types become part of the WarrenLogging module, removing
# the need for `import Logging` (which fails to resolve in Xcode 26.4
# due to a swift-driver SPM-consumer-path regression).
#
# Also drops the swift-log SPM package_reference + WarrenLogging's
# Logging packageProductDependency since they're no longer consumed.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/add-vendored-swiftlog.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

VENDOR_FILES = %w[
  WarrenLogging/Vendor/SwiftLog/Locks.swift
  WarrenLogging/Vendor/SwiftLog/Logging.swift
  WarrenLogging/Vendor/SwiftLog/LogHandler.swift
  WarrenLogging/Vendor/SwiftLog/MetadataProvider.swift
].freeze

target = project.targets.find { |t| t.name == "WarrenLogging" }
unless target
  warn "WarrenLogging target not found"
  exit 1
end

# Find or create the Vendor/SwiftLog group.
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

VENDOR_FILES.each do |rel_path|
  existing = project.files.find { |f| f.real_path.to_s == File.join(SCRIPT_DIR, rel_path) }
  if existing
    unless target.source_build_phase.files_references.include?(existing)
      target.add_file_references([existing])
      added << "wire: #{rel_path}"
    else
      skipped << rel_path
    end
    next
  end
  parent_dir = File.dirname(rel_path)
  parent_group = find_or_create_group(project, parent_dir)
  file_ref = parent_group.new_reference(File.basename(rel_path))
  target.add_file_references([file_ref])
  added << "add+wire: #{rel_path}"
end

# Drop swift-log packageProductDependency from WarrenLogging.
if target.respond_to?(:package_product_dependencies)
  target.package_product_dependencies.dup.each do |dep|
    next unless dep.product_name == "Logging"
    target.package_product_dependencies.delete(dep)
    added << "drop dep: Logging"
  end
end

# Drop swift-log entry from WarrenLogging Frameworks (link) phase.
if target.respond_to?(:frameworks_build_phase) && target.frameworks_build_phase
  target.frameworks_build_phase.files.dup.each do |build_file|
    name = build_file.respond_to?(:display_name) ? build_file.display_name : nil
    next unless name == "Logging" || name == "Logging in Frameworks"
    target.frameworks_build_phase.files.delete(build_file)
    added << "drop link: #{name}"
  end
end

# Drop swift-log package_reference from project (the only consumer
# was WarrenLogging which we just unwired).
project.root_object.package_references.dup.each do |pkg|
  source = pkg.respond_to?(:repositoryURL) ? pkg.repositoryURL : nil
  next unless source && source.to_s.include?("swift-log")
  project.root_object.package_references.delete(pkg)
  added << "drop package_reference: swift-log"
end

if added.empty? && skipped.empty?
  puts "No changes needed."
else
  project.save
  puts "Vendored swift-log applied :"
  added.each { |e| puts "  + #{e}" }
  unless skipped.empty?
    puts "Skipped (already wired):"
    skipped.each { |e| puts "  = #{e}" }
  end
end
