local functions = {}

function on_pc(address, fn)
	functions[address] = fn
	debug.set_breakpoint(address)
end

function debug.on_breakpoint(address)
	local fn = functions[address]
	if fn then
		fn(address)
	end
end

on_pc(0x080E518C, function()
	print("r0", cpu.read_register("r0"))
end)
