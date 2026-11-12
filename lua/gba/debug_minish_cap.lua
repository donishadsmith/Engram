emulator.pause()
emulator.reset()
--debugger.set_watchpoint(0x10000000, {end_address = 0xFFFFFFFE, on = "read", width = "halfword"})
--debugger.set_watchpoint(0x00000004, {end_address = 0x00003FFC, on = "read", width = "word"})
debugger.set_watchpoint(0x00000004, {end_address = 0x00003FFE, on = "read", width = "halfword"})

function debugger.on_watchpoint(hit)
  print(string.format("fetch %08X  pc=%08X, elapsed_cycles=%d", hit.address, hit.pc, hit.cycle))
  for _, name in ipairs(cpu.register_names()) do
    print(name, string.format("%08X", cpu.read_register(name)))
  end
end

emulator.resume()
