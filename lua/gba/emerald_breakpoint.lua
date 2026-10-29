debug.set_breakpoint(0x080E5132, { pause = false })

function debug.on_breakpoint()
	for index = 0, 15 do
		local register = string.format("r%d", index)
		print(string.format("%-4s %08X", register, cpu.read_register(register)))
	end

	print(string.format("cpsr %08X", cpu.read_register("cpsr")))
end
