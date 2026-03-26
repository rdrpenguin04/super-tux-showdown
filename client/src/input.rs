use bevy::prelude::*;

#[derive(Reflect, Clone, Copy, Debug)]
pub enum InputAction {
    Jump,
    FastFall,
}

#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
pub struct ActionBuffer {
    pub jump: ActionState<true>,
    pub fast_fall: ActionState<false>,
}

impl ActionBuffer {
    pub fn tick(&mut self) {
        self.jump.tick();
        self.fast_fall.tick();
    }

    pub fn state_of(&self, action: InputAction) -> ActionStateInner {
        match action {
            InputAction::Jump => *self.jump,
            InputAction::FastFall => *self.fast_fall,
        }
    }

    pub fn try_consume(&mut self, allowed: &[InputAction]) -> Option<InputAction> {
        for action in allowed {
            if self.state_of(*action).in_window() {
                self.clear();
                return Some(*action);
            }
        }
        None
    }

    pub fn clear(&mut self) {
        *self.jump = ActionStateInner::ReleasedFor(EVER);
        *self.fast_fall = ActionStateInner::ReleasedFor(EVER);
    }
}

#[derive(Clone, Copy, Reflect, Debug, Default, Deref, DerefMut)]
pub struct ActionState<const HELD: bool>(pub ActionStateInner);

impl<const HELD: bool> ActionState<HELD> {
    pub const JUST_PRESSED: Self = Self(ActionStateInner::PressedFor(JUST));
    pub const JUST_RELEASED: Self = Self(ActionStateInner::ReleasedFor(JUST));

    pub fn tick(&mut self) {
        if HELD {
            self.tick_held();
        } else {
            self.tick_tap();
        }
    }
}

#[derive(Clone, Copy, Reflect, Debug)]
pub enum ActionStateInner {
    PressedFor(ActionTime),
    ReleasedFor(ActionTime),
}

impl Default for ActionStateInner {
    fn default() -> Self {
        Self::ReleasedFor(EVER)
    }
}

pub const BUFFER_TIME: u8 = 10;

impl ActionStateInner {
    pub const JUST_PRESSED: Self = Self::PressedFor(JUST);
    pub const JUST_RELEASED: Self = Self::ReleasedFor(JUST);

    pub fn tick_tap(&mut self) {
        match self {
            Self::PressedFor(_) => *self = Self::JUST_RELEASED,
            Self::ReleasedFor(x) => x.tick(),
        }
    }

    pub fn tick_held(&mut self) {
        match self {
            Self::PressedFor(x) => x.tick(),
            Self::ReleasedFor(x) => x.tick(),
        }
    }

    pub fn in_window(&self) -> bool {
        match self {
            Self::PressedFor(_) => true,
            Self::ReleasedFor(x) => x.0 <= BUFFER_TIME,
        }
    }

    pub fn press(&mut self) {
        match self {
            Self::PressedFor(_) => {}
            Self::ReleasedFor(_) => *self = Self::JUST_PRESSED,
        }
    }

    pub fn release(&mut self) {
        match self {
            Self::PressedFor(_) => *self = Self::JUST_RELEASED,
            Self::ReleasedFor(_) => {}
        }
    }

    pub fn is_pressed(&self) -> bool {
        matches!(self, Self::PressedFor(_))
    }

    pub fn is_released(&self) -> bool {
        matches!(self, Self::ReleasedFor(_))
    }
}

#[derive(Clone, Copy, Reflect, Debug)]
pub struct ActionTime(pub u8);

impl ActionTime {
    pub const JUST: Self = Self(0);
    pub const EVER: Self = Self(255);

    pub fn tick(&mut self) {
        self.0 = self.0.saturating_add(1);
    }
}

pub const JUST: ActionTime = ActionTime::JUST;
pub const EVER: ActionTime = ActionTime::EVER;

#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct HeldInputs {
    pub direction: f32,
    pub jump: bool,
}

#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct InputConfig {
    // TODO: fields. Currently this structure is only a marker that a player can be controlled
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        RunFixedMainLoop,
        poll_inputs.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedPreUpdate, tick_buffers);
}

macro_rules! input_buffer {
    ($input:expr, $action_buffer:expr, $key:expr, $field:ident) => {
        if $input.just_pressed($key) {
            $action_buffer.$field.press();
        }
        if $input.just_released($key) {
            $action_buffer.$field.release();
        }
    };
}

pub fn poll_inputs(
    input: Res<ButtonInput<KeyCode>>,
    mut buffers: Query<(&mut HeldInputs, &mut ActionBuffer, &InputConfig)>,
) {
    use KeyCode::*;

    for (mut held, mut action_buffer, _config) in &mut buffers {
        held.direction = if input.pressed(KeyA) && !input.pressed(KeyD) {
            -1.0
        } else if input.pressed(KeyD) && !input.pressed(KeyA) {
            1.0
        } else {
            0.0
        };

        held.jump = input.pressed(KeyW);

        input_buffer!(input, action_buffer, KeyW, jump);
        input_buffer!(input, action_buffer, KeyS, fast_fall);
}
}

pub fn tick_buffers(mut action_buffers: Query<&mut ActionBuffer>) {
    for mut buffer in &mut action_buffers {
        buffer.tick();
    }
}
