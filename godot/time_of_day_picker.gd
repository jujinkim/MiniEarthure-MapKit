extends OptionButton
## Shared environment presentation; the numeric hour remains the stored contract.
signal value_changed(value: float)
const HOURS := [9,12,17,18,21,6]
const LABELS := ["Morning · 09:00","Noon · 12:00","Evening · 17:00","Sunset · 18:00","Night · 21:00","Dawn · 06:00"]
var _value := 12.0
var value: float:
	get: return _value
	set(next):
		_value=next
		_refresh_selection()
func _init() -> void:
	fit_to_longest_item=false
	custom_minimum_size.y=44
	for index in HOURS.size(): add_item(String(TranslationServer.translate(LABELS[index])),HOURS[index]*60)
	_refresh_selection()
	item_selected.connect(func(index:int):
		_value=float(HOURS[index])
		value_changed.emit(_value))
func _refresh_selection() -> void:
	if item_count==0: return
	var index:=HOURS.find(_value)
	select(index)
	if index<0:
		# Display arbitrary authored minutes without rounding them to a preset.
		var minute:=posmod(roundi(_value*60.0),1440)
		text="%02d:%02d"%[minute/60,minute%60]
func _notification(what: int) -> void:
	if what==NOTIFICATION_TRANSLATION_CHANGED:
		for index in item_count: set_item_text(index,String(TranslationServer.translate(LABELS[index])))
		_refresh_selection()
