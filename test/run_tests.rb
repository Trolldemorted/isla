#!/usr/bin/env ruby

require 'open3'
include Process

$TEST_DIR = File.expand_path(File.dirname(__FILE__))

class String
  def colorize(color_code)
    "\e[#{color_code}m#{self}\e[0m"
  end

  def red
    colorize(91)
  end

  def green
    colorize(92)
  end

  def yellow
    colorize(93)
  end

  def blue
    colorize(94)
  end

  def cyan
    colorize(96)
  end
end

class Dir
  def self.chunks(dir, n, &block)
    Dir.entries(dir).each_slice(n).each do |chunk|
      chunk.each { |file| fork { yield file } }
      statuses = Process.waitall
      statuses.each do |status|
        exit 1 if status[1].exitstatus != 0
      end
    end
  end
end

def step(str, *extra)
  expected = extra.fetch(0, 0)
  stdout, stderr, status = Open3.capture3("#{str}")
  if status.exitstatus != expected then
    puts <<OUTPUT
#{"Failed".red} (expected #{expected}, got: #{status}): #{str}
#{"stdout".cyan}: #{stdout}
#{"stderr".blue}: #{stderr}
OUTPUT
    exit 1
  end
end

def step_print(str)
  if !system str then
    puts <<OUTPUT
#{"Failed".red}: #{str}
OUTPUT
    exit 1
  end
end

def chdir_relative(path)
  Dir.chdir(File.expand_path(File.join($TEST_DIR, path)))
end

def wget(url, file)
  return if File.file?(file)

  system "wget -O #{file} #{url}"
end

def run_tests()
  chdir_relative "../isla-sail"
  exit 1 if !system "make"
  isla_sail = File.expand_path(File.join($TEST_DIR, "../isla-sail/isla-sail"))
  exit 1 if !File.file?(isla_sail)

  chdir_relative "../isla-litmus"
  exit 1 if !system "make"
  isla_litmus = File.expand_path(File.join($TEST_DIR, "../isla-litmus/isla-litmus"))
  exit 1 if !File.file?(isla_litmus)
  
  chdir_relative ".."
  exit 1 if !system "cargo build --release"

  chdir_relative "property"
  ["", "129"].each do |suffix|
    isla_property = File.expand_path(File.join($TEST_DIR, "../target/release/isla-property#{suffix}"))
    exit 1 if !File.file?(isla_property)
    
    puts "Running tests [#{suffix}]:".blue
    
    Dir.chunks ".", 12 do |file|
      next if file !~ /.+\.sail$/

      basename = File.basename(file, ".*")

      building = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      step("#{isla_sail} #{file} include/config.sail -o #{basename}")
      starting = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      if File.extname(basename) == ".unsat" then
        step("LD_LIBRARY_PATH=..:$LD_LIBRARY_PATH #{isla_property} -A #{basename}.ir -p prop -L lin -T 2 -C ../../configs/plain.toml")
      else
        step("LD_LIBRARY_PATH=..:$LD_LIBRARY_PATH #{isla_property} -A #{basename}.ir -p prop -T 2 -C ../../configs/plain.toml", 1)
      end
      ending = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      build_time = (starting - building) * 1000
      time = (ending - starting) * 1000
      puts "#{file}".ljust(40).concat("#{"ok".green} (#{build_time.to_i}ms/#{time.to_i}ms)\n")
    end
  end

  isla_axiomatic = File.expand_path(File.join($TEST_DIR, "../target/release/isla-axiomatic"))
  exit 1 if !File.file?(isla_axiomatic)

  chdir_relative "."
  
  arch = "axiomatic/riscv64.ir"
  wget("https://raw.githubusercontent.com/rems-project/isla-snapshots/master/riscv64.ir", arch)
  
  step_print("LD_LIBRARY_PATH=..:$LD_LIBRARY_PATH #{isla_axiomatic} -A #{arch} -C ../configs/riscv64_ubuntu.toml -m ../web/client/dist/riscv.cat -t axiomatic/tests --refs axiomatic/refs")
end

run_tests
