use bevy::prelude::*;

#[derive(Reflect, Clone, Copy, Debug)]
pub enum InputAction {
    Jump,
}

#[derive(Resource, Reflect, Debug, Default)]
#[reflect(Resource)]
pub struct ActionBuffer {
    pub jump: ActionState,
}

impl ActionBuffer {
    pub fn tick(&mut self) {
        self.jump.tick();
    }

    pub fn state_of(&self, action: InputAction) -> ActionState {
        match action {
            InputAction::Jump => self.jump,
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
        self.jump = ActionState::ReleasedFor(EVER);
    }
}

#[derive(Clone, Copy, Reflect, Debug)]
pub enum ActionState {
    PressedFor(ActionTime),
    ReleasedFor(ActionTime),
}

impl Default for ActionState {
    fn default() -> Self {
        Self::ReleasedFor(EVER)
    }
}

pub const BUFFER_TIME: u8 = 10;

impl ActionState {
    pub const JUST_PRESSED: Self = Self::PressedFor(JUST);
    pub const JUST_RELEASED: Self = Self::ReleasedFor(JUST);

    pub fn tick(&mut self) {
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

#[derive(Resource, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Resource)]
pub struct HeldInputs {
    pub direction: f32,
    pub jump: bool,
    pub fast_fall: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ActionBuffer>()
        .init_resource::<HeldInputs>();

    app.add_systems(
        RunFixedMainLoop,
        poll_inputs.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(FixedPreUpdate, tick_buffer);
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
    mut held: ResMut<HeldInputs>,
    mut action_buffer: ResMut<ActionBuffer>,
) {
    use KeyCode::*;

    held.direction = if input.pressed(KeyA) && !input.pressed(KeyD) {
        -1.0
    } else if input.pressed(KeyD) && !input.pressed(KeyA) {
        1.0
    } else {
        0.0
    };

    held.jump = input.pressed(KeyW);
    held.fast_fall = input.pressed(KeyS);

    input_buffer!(input, action_buffer, KeyW, jump);
}

pub fn tick_buffer(mut action_buffer: ResMut<ActionBuffer>) {
    action_buffer.tick();
}
