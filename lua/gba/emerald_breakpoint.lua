set_breakpoint(0x080E518C, { pause = false })

function on_breakpoint(address)
	for index = 0, 15 do
		local register = string.format("r%d", index)
		print(string.format("%-4s %08X", register, read_cpu_register(register)))
	end

	print(string.format("cpsr %08X", read_cpu_register("cpsr")))
end
