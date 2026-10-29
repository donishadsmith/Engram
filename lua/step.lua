emulator.pause()

for index = 1, 100 do
	emulator.step_frame()
	-- emulator.step_instruction()
	if index == 100 then
		console.log("final step reached")
	end
end
