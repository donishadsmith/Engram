local step = 0
function emulator.on_frame()
    step = step + 1
    if step % 16 < 8 then
		controller.set({ up = true })
	else
		controller.set({ down = true })
	end
end
