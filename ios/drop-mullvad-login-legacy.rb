#!/usr/bin/env ruby
# frozen_string_literal: true

# Drops the Mullvad account-number login UI cluster from the WarrenVPN
# target's source build phase. P3.1 replaced it with the wallet
# Create/Restore login (WarrenWalletLoginCoordinator), so these files are
# dead. Following the team's drop pattern, the files stay on disk (file
# references remain in the project) and are only removed from the build.
#
# Usage: ruby ios/drop-mullvad-login-legacy.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

DROP = [
  "WarrenVPN/Coordinators/LoginCoordinator.swift",
  "WarrenVPN/View controllers/Login/LoginViewController.swift",
  "WarrenVPN/View controllers/Login/LoginInteractor.swift",
  "WarrenVPN/View controllers/Login/LoginContentView.swift",
  "WarrenVPN/View controllers/Login/AccountInputGroupView.swift",
  "WarrenVPN/View controllers/Login/AccountTextField.swift",
  "WarrenVPN/View controllers/Login/AccessMethodInvalidView.swift",
].freeze

root = File.expand_path(SCRIPT_DIR)
dropped = []

project.targets.each do |target|
  next unless target.respond_to?(:source_build_phase)
  target.source_build_phase.files.dup.each do |bf|
    rp = (bf.file_ref&.real_path&.to_s rescue nil)
    next unless rp
    rel = rp.sub(root + "/", "")
    if DROP.include?(rel)
      bf.remove_from_project
      dropped << "#{rel} (#{target.name})"
    end
  end
end

if dropped.empty?
  puts "Mullvad login cluster already dropped from all build phases. No changes."
else
  project.save
  puts "Dropped #{dropped.size} build-phase entries:"
  dropped.each { |d| puts "  - #{d}" }
end
